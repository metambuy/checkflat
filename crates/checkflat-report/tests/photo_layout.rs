//! The photo layout of the report (D-029, replacing the single column of D-025 by Martín's
//! decision of 2026-10-26): two photos per row, three photos a row of two and a row of one,
//! each photo at most 80 mm wide and 70 mm tall with its caption under it, a row never split
//! across pages, and the "Images" label staying with the first row.
//!
//! The tests read where the template put things from the PDF itself: the content stream of a
//! page gives every image's box (`cm` + `Do`) and the origin of every line of text (`cm` +
//! `Tm`), in points from the page's bottom-left.
mod common;

use std::collections::HashSet;

use checkflat_core::i18n::Lang;
use checkflat_report::{collect, prepare, render};
use lopdf::content::Content;
use lopdf::{Document, Object, ObjectId};

const MM: f64 = 72.0 / 25.4;
/// The template's page: A4, 20 mm side margins, 18 mm top and bottom.
const MARGIN_X: f64 = 20.0 * MM;
const MARGIN_Y: f64 = 18.0 * MM;
const A4_H: f64 = 841.8898;
/// A photo cell: at most 80 × 70 mm with its 2 pt frame; the two columns are those of the plan row.
const CELL_W: f64 = 80.0 * MM;
const CELL_H: f64 = 70.0 * MM;
const GUTTER: f64 = 6.0 * MM;
const INSET: f64 = 2.0;
/// Positions in the content stream are f32 and Typst rounds to a few decimals.
const TOL: f64 = 0.6;

type Matrix = [f64; 6];
const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `m` applied first, then `n` (PDF's row-vector convention: `cm` sets CTM = M × CTM).
fn mul(m: Matrix, n: Matrix) -> Matrix {
    [
        m[0] * n[0] + m[1] * n[2],
        m[0] * n[1] + m[1] * n[3],
        m[2] * n[0] + m[3] * n[2],
        m[2] * n[1] + m[3] * n[3],
        m[4] * n[0] + m[5] * n[2] + n[4],
        m[4] * n[1] + m[5] * n[3] + n[5],
    ]
}

fn apply(m: Matrix, (x, y): (f64, f64)) -> (f64, f64) {
    (x * m[0] + y * m[2] + m[4], x * m[1] + y * m[3] + m[5])
}

#[derive(Debug, Clone, Copy)]
enum Item {
    /// An image's box; `top` is the distance from the page's top edge.
    Image { x: f64, top: f64, w: f64, h: f64, jpeg: bool },
    /// The origin (baseline start) of a line of text.
    Text { x: f64, top: f64 },
}

fn numbers(op: &lopdf::content::Operation) -> Matrix {
    let mut m = IDENTITY;
    for (slot, o) in m.iter_mut().zip(&op.operands) {
        *slot = f64::from(o.as_float().expect("a number"));
    }
    m
}

/// The names of the page's JPEG (`DCTDecode`) images: the photos. Crops and overviews are PNG.
fn jpeg_names(doc: &Document, page: ObjectId) -> HashSet<Vec<u8>> {
    let (inline, ids) = doc.get_page_resources(page).unwrap();
    let dicts = inline.into_iter().chain(ids.iter().map(|&id| doc.get_dictionary(id).unwrap()));
    let mut names = HashSet::new();
    for res in dicts {
        let Ok(xobjects) = res.get(b"XObject").and_then(|o| doc.dereference(o)).and_then(|(_, o)| o.as_dict()) else { continue };
        for (name, obj) in xobjects.iter() {
            let stream = doc.dereference(obj).unwrap().1.as_stream().unwrap();
            let is_jpeg = match stream.dict.get(b"Filter") {
                Ok(Object::Name(n)) => n == b"DCTDecode",
                Ok(Object::Array(a)) => a.iter().any(|f| matches!(f, Object::Name(n) if n == b"DCTDecode")),
                _ => false,
            };
            if is_jpeg {
                names.insert(name.clone());
            }
        }
    }
    names
}

/// Everything drawn on page `no` (1-based), in drawing order.
fn page_items(doc: &Document, no: u32) -> Vec<Item> {
    let page = doc.get_pages()[&no];
    let height = {
        let media = doc.get_dictionary(page).unwrap().get(b"MediaBox").unwrap().as_array().unwrap();
        f64::from(media[3].as_float().unwrap())
    };
    let jpegs = jpeg_names(doc, page);
    let ops = Content::decode(&doc.get_page_content(page)).unwrap().operations;
    let (mut ctm, mut stack) = (IDENTITY, Vec::<Matrix>::new());
    let (mut tm, mut shown) = (IDENTITY, true);
    let mut items = Vec::new();
    for op in &ops {
        match op.operator.as_str() {
            "q" => stack.push(ctm),
            "Q" => ctm = stack.pop().unwrap_or(IDENTITY),
            "cm" => ctm = mul(numbers(op), ctm),
            "BT" => (tm, shown) = (IDENTITY, false),
            "Tm" => tm = numbers(op),
            "Tj" | "TJ" if !shown => {
                shown = true;
                let (x, y) = apply(mul(tm, ctm), (0.0, 0.0));
                items.push(Item::Text { x, top: height - y });
            }
            "Do" => {
                let name = op.operands[0].as_name().unwrap().to_vec();
                let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)].map(|c| apply(ctm, c));
                let (x0, x1) = corners.iter().fold((f64::MAX, f64::MIN), |(lo, hi), c| (lo.min(c.0), hi.max(c.0)));
                let (y0, y1) = corners.iter().fold((f64::MAX, f64::MIN), |(lo, hi), c| (lo.min(c.1), hi.max(c.1)));
                items.push(Item::Image { x: x0, top: height - y1, w: x1 - x0, h: y1 - y0, jpeg: jpegs.contains(&name) });
            }
            _ => {}
        }
    }
    items
}

/// A photo as the PDF shows it: the frame (image + 2 pt on every side) and the caption's origin.
#[derive(Debug, Clone, Copy)]
struct Photo {
    page: u32,
    x: f64,
    top: f64,
    w: f64,
    h: f64,
    caption: Option<(f64, f64)>,
}

impl Photo {
    fn bottom(&self) -> f64 {
        self.top + self.h
    }
}

/// Every photo of the report in drawing order, with the first line of text drawn after it, which
/// is its caption when it is on the same page.
fn photos(pdf: &[u8]) -> Vec<Photo> {
    let doc = Document::load_mem(pdf).unwrap();
    let mut out = Vec::new();
    for no in doc.get_pages().keys().copied() {
        let items = page_items(&doc, no);
        for (i, item) in items.iter().enumerate() {
            let Item::Image { x, top, w, h, jpeg: true } = *item else { continue };
            let caption = items[i + 1..].iter().find_map(|n| match *n {
                Item::Text { x, top } => Some((x, top)),
                Item::Image { .. } => None,
            });
            out.push(Photo { page: no, x: x - INSET, top: top - INSET, w: w + 2.0 * INSET, h: h + 2.0 * INSET, caption });
        }
    }
    out
}

fn render_pdf(conn: &checkflat_core::rusqlite::Connection, data: &std::path::Path, pid: &str) -> Vec<u8> {
    let src = collect(conn, data, pid, Lang::En, "UTC", 0).unwrap();
    render(&prepare(data, src).unwrap()).unwrap().pdf
}

fn page_texts(pdf: &[u8]) -> Vec<String> {
    let doc = Document::load_mem(pdf).unwrap();
    doc.get_pages().keys().map(|&n| doc.extract_text(&[n]).unwrap()).collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= TOL
}

/// A photo of the given kind: `L` landscape, `P` portrait, `W` wide and short (all unique, 1600 px
/// on the long side).
fn photo(data: &std::path::Path, kind: char, n: u32) -> checkflat_core::photos::PhotoInput {
    let (w, h) = match kind {
        'P' => (1200, 1600),
        'W' => (1600, 500),
        _ => (1600, 1200),
    };
    common::photo_seeded(data, w, h, &format!("2026-01-01T{:02}:00:00.000Z", n % 24), n)
}

/// Three photos (landscape, portrait, landscape) are a row of two and a row of one: the first two
/// side by side on the plan row's two columns, tops aligned, the third under the first; every
/// photo fits 80 × 70 mm and has its caption under it, on the photo's left edge.
#[test]
fn three_photos_are_a_row_of_two_and_a_row_of_one() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = common::open(data);
    let pid = common::project(&conn, "Rows", "");
    // No pyramid: the plan row shows two placeholders, so the head is short and everything is on
    // one page, and the only images in the PDF are the photos.
    let plan = common::plan(&conn, data, &pid, "P", common::A4);
    common::observation(&conn, data, &plan, 0.5, 0.5, "three photos", &[photo(data, 'L', 1), photo(data, 'P', 2), photo(data, 'L', 3)]);
    let pdf = render_pdf(&conn, data, &pid);

    let ph = photos(&pdf);
    assert_eq!(ph.len(), 3, "{ph:?}");
    assert!(ph.iter().all(|p| p.page == 2), "cover, then one page: {ph:?}");
    let (a, b, c) = (ph[0], ph[1], ph[2]);

    // Columns: the left edge of the text column, and 80 + 6 mm further right.
    assert!(near(a.x, MARGIN_X), "first photo on the left column: {a:?}");
    assert!(near(b.x, MARGIN_X + CELL_W + GUTTER), "second photo on the right column: {b:?}");
    assert!(near(c.x, MARGIN_X), "third photo under the first: {c:?}");
    // Rows: the first two share a top (the portrait beside the landscape does not stretch or
    // shift it); the third starts below the taller of them and its caption.
    assert!(near(a.top, b.top), "one row: {a:?} {b:?}");
    let row_bottom = a.bottom().max(b.bottom());
    assert!(c.top > row_bottom + 8.0, "a gap for the caption and the row spacing: {c:?} vs {row_bottom}");
    assert!(c.top < row_bottom + 40.0, "but no more than a caption and 4 mm: {c:?} vs {row_bottom}");

    // Size: the same maxima as the single column had.
    for p in [a, b, c] {
        assert!(p.w <= CELL_W + TOL && p.h <= CELL_H + TOL, "at most 80 × 70 mm: {p:?}");
    }
    assert!(near(a.w, CELL_W) && near(c.w, CELL_W), "landscape photos use the full 80 mm: {a:?}");
    assert!(near(b.h, CELL_H) || near(b.h, CELL_H - 1.5), "the portrait photo is 70 mm tall (within its frame): {b:?}");
    assert!(b.w < a.w, "and narrower than a landscape one: {b:?}");

    // Captions: a line of text under each photo, starting at the photo's left edge, close below it.
    for p in [a, b, c] {
        let (x, top) = p.caption.expect("a caption after the photo");
        assert!(near(x, p.x), "caption on the photo's left edge: {p:?} {x}");
        assert!(top > p.bottom() && top < p.bottom() + 16.0, "caption right under the photo: {p:?} {top}");
    }
    let text = page_texts(&pdf).concat();
    let order: Vec<usize> = ["1. 01.01.2026 01:00", "2. 01.01.2026 02:00", "3. 01.01.2026 03:00"].iter().map(|c| text.find(c).unwrap_or_else(|| panic!("{c} in {text}"))).collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "captions numbered in order: {text}");
}

/// One photo is a row of one on the left column; two are a single row.
#[test]
fn one_and_two_photos_are_single_rows() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = common::open(data);
    let pid = common::project(&conn, "Few", "");
    let plan = common::plan(&conn, data, &pid, "P", common::A4);
    common::observation(&conn, data, &plan, 0.4, 0.4, "one", &[photo(data, 'L', 1)]);
    common::observation(&conn, data, &plan, 0.6, 0.6, "two", &[photo(data, 'L', 2), photo(data, 'P', 3)]);
    let ph = photos(&render_pdf(&conn, data, &pid));
    assert_eq!(ph.len(), 3);
    // Newest first: the two-photo observation comes first.
    assert!(near(ph[0].x, MARGIN_X) && near(ph[1].x, MARGIN_X + CELL_W + GUTTER) && near(ph[0].top, ph[1].top), "{ph:?}");
    assert!(near(ph[2].x, MARGIN_X), "a lone photo on the left column: {ph:?}");
}

/// Nine observations of three photos with descriptions of different lengths, so the page breaks
/// fall at different places in them. On every page: a row's two photos are on one page and one
/// row; the third photo is on the same page under them or at the top of the next; nothing is
/// drawn into the margins; the "Images" label is on the page of its first row.
#[test]
fn a_row_is_never_split_across_pages_and_the_label_stays_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path();
    let conn = common::open(data);
    let pid = common::project(&conn, "Flow", "");
    let plan = common::plan(&conn, data, &pid, "P", common::A4);
    // Wide, short photos (a row is about 30 mm with its caption, an observation about 140 mm of
    // the 261 mm page) and descriptions of different lengths (sentences of about one line), so
    // that the page breaks fall in different places: before a head, between the description and
    // the first row, and between the two rows.
    const SENTENCES: [usize; 9] = [2, 6, 10, 4, 8, 12, 3, 7, 1];
    for (i, sentences) in SENTENCES.iter().enumerate() {
        let n = (i * 3) as u32;
        let description = format!("Q{i:02}Q {}", "Fissura horizontal na parede da sala, junto ao rodapé, com cerca de 1,2 m de comprimento. ".repeat(*sentences));
        common::observation(&conn, data, &plan, 0.5, 0.5, &description, &[photo(data, 'W', n + 1), photo(data, 'W', n + 2), photo(data, 'W', n + 3)]);
    }
    let pdf = render_pdf(&conn, data, &pid);
    let ph = photos(&pdf);
    assert_eq!(ph.len(), 27);
    let texts = page_texts(&pdf);

    // The page of each observation's description, in drawing order, from its marker.
    let desc_page: Vec<u32> = {
        let mut found: Vec<(usize, u32)> = Vec::new();
        for (p, t) in texts.iter().enumerate() {
            for i in 0..SENTENCES.len() {
                if let Some(at) = t.find(&format!("Q{i:02}Q")) {
                    found.push((at + p * 1_000_000, p as u32 + 1));
                }
            }
        }
        found.sort();
        found.into_iter().map(|(_, page)| page).collect()
    };
    assert_eq!(desc_page.len(), SENTENCES.len(), "every description found once: {texts:?}");

    let (mut row_moved, mut single_flowed) = (0, 0);
    for (k, g) in ph.chunks(3).enumerate() {
        let (a, b, c) = (g[0], g[1], g[2]);
        assert_eq!(a.page, b.page, "observation {k}: a row's photos are on one page: {g:?}");
        assert!(near(a.top, b.top) && near(b.x, MARGIN_X + CELL_W + GUTTER), "observation {k}: and side by side: {g:?}");
        if c.page == a.page {
            assert!(c.top > a.bottom().max(b.bottom()) && near(c.x, MARGIN_X), "observation {k}: the third under the first: {g:?}");
        } else {
            single_flowed += 1;
            assert_eq!(c.page, a.page + 1, "observation {k}: or at the top of the next page: {g:?}");
            assert!(near(c.x, MARGIN_X) && c.top < MARGIN_Y + 60.0, "observation {k}: {g:?}");
        }
        if a.page > desc_page[k] {
            row_moved += 1;
        }
        assert!(a.page >= desc_page[k], "photos never come before the description: {k}");
    }
    // Margins: no photo and no caption reaches into the bottom margin (a row that does not fit
    // moves whole instead of being clipped). The page number lives in that margin by design.
    for p in &ph {
        let (_, caption) = p.caption.expect("a caption on the same page");
        assert!(p.bottom() <= A4_H - MARGIN_Y + TOL && caption <= A4_H - MARGIN_Y + TOL, "page {}: {p:?} below the bottom margin", p.page);
    }
    // The label stays with the first row: the labels on a page are those of the observations
    // whose first row is on it (an orphaned label would be counted on the page before).
    for (p, t) in texts.iter().enumerate() {
        let rows_here = ph.chunks(3).filter(|g| g[0].page == p as u32 + 1).count();
        assert_eq!(t.matches("Images").count(), rows_here, "page {}: labels vs first rows: {t}", p + 1);
    }
    // The fixture really exercises the rules: some first row moved to the page after its
    // description (with the label), and some lone photo flowed to the next page.
    assert!(row_moved > 0 && single_flowed > 0, "row_moved {row_moved}, single_flowed {single_flowed}; adjust SENTENCES");
}
