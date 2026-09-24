//! printpdf engine: every layout decision (line breaking, positions, spacing) is manual.
use crate::{Error, Report, FONT_BOLD, FONT_REGULAR};
use printpdf::*;

const PAGE_W_MM: f32 = 210.0;
const PAGE_H_MM: f32 = 297.0;
const MARGIN_MM: f32 = 20.0;
const CONTENT_W_MM: f32 = PAGE_W_MM - 2.0 * MARGIN_MM;
const MM_TO_PT: f32 = 72.0 / 25.4;

fn pt(mm: f32) -> Pt {
    Pt(mm * MM_TO_PT)
}
/// y coordinate (pt, from bottom) of a baseline `mm` from the top edge.
fn y_top(mm: f32) -> Pt {
    Pt((PAGE_H_MM - mm) * MM_TO_PT)
}
fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(Rgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, None))
}

/// Advance width of `s` in pt at `size`, from the font's hmtx table.
fn text_width(font: &ParsedFont, s: &str, size: f32) -> f32 {
    let upem = font.units_per_em.max(1) as f32;
    s.chars()
        .map(|c| {
            font.lookup_glyph_index(c as u32)
                .and_then(|g| font.get_glyph_width(g))
                .unwrap_or((upem * 0.5) as u16) as f32
        })
        .sum::<f32>()
        * size
        / upem
}

/// Greedy word wrap. Returns lines; no hyphenation, no justification.
fn wrap(font: &ParsedFont, text: &str, size: f32, max_w: f32) -> Vec<String> {
    let space = text_width(font, " ", size);
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut w = 0.0;
    for word in text.split_whitespace() {
        let ww = text_width(font, word, size);
        if !line.is_empty() && w + space + ww > max_w {
            lines.push(std::mem::take(&mut line));
            w = 0.0;
        }
        if !line.is_empty() {
            line.push(' ');
            w += space;
        }
        line.push_str(word);
        w += ww;
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn text_block(font: &FontId, size: f32, leading: f32, x: Pt, y: Pt, lines: &[String]) -> Vec<Op> {
    let mut ops = vec![
        Op::StartTextSection,
        Op::SetFont { font: PdfFontHandle::External(font.clone()), size: Pt(size) },
        Op::SetLineHeight { lh: Pt(leading) },
        Op::SetTextCursor { pos: Point { x, y } },
    ];
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            ops.push(Op::AddLineBreak);
        }
        ops.push(Op::ShowText { items: vec![TextItem::Text(l.clone())] });
    }
    ops.push(Op::EndTextSection);
    ops
}

pub fn render(report: &Report) -> Result<Vec<u8>, Error> {
    let mut doc = PdfDocument::new("Checkflat report");
    let mut fw = Vec::new();
    let regular = ParsedFont::from_bytes(FONT_REGULAR, 0, &mut fw).ok_or("parse regular font")?;
    let bold = ParsedFont::from_bytes(FONT_BOLD, 0, &mut fw).ok_or("parse bold font")?;
    let regular_id = doc.add_font(&regular);
    let bold_id = doc.add_font(&bold);

    let mut warnings = Vec::new();
    let image = RawImage::decode_from_bytes(report.image, &mut warnings).map_err(|e| format!("decode image: {e}"))?;
    let image_id = doc.add_image(&image);

    let navy = rgb(0x14, 0x3c, 0x78);
    let grey = rgb(0x80, 0x80, 0x80);
    let black = rgb(0x1a, 0x1a, 0x1a);
    let x0 = pt(MARGIN_MM);
    let content_w_pt = CONTENT_W_MM * MM_TO_PT;

    let mut ops: Vec<Op> = Vec::new();

    // Header: title, subtitle, rule.
    let mut cursor_mm = MARGIN_MM + 7.0; // baseline of the 20pt title
    ops.push(Op::SetFillColor { col: navy.clone() });
    ops.extend(text_block(&bold_id, 20.0, 24.0, x0, y_top(cursor_mm), &[report.title.to_string()]));
    cursor_mm += 5.5;
    ops.push(Op::SetFillColor { col: grey.clone() });
    ops.extend(text_block(&regular_id, 10.0, 12.0, x0, y_top(cursor_mm), &[report.subtitle.to_string()]));
    cursor_mm += 3.0;
    ops.push(Op::SetOutlineColor { col: navy.clone() });
    ops.push(Op::SetOutlineThickness { pt: Pt(1.5) });
    ops.push(Op::DrawLine {
        line: Line {
            points: vec![
                LinePoint { p: Point { x: x0, y: y_top(cursor_mm) }, bezier: false },
                LinePoint { p: Point { x: pt(PAGE_W_MM - MARGIN_MM), y: y_top(cursor_mm) }, bezier: false },
            ],
            is_closed: false,
        },
    });

    // Image: 120 mm wide, aspect preserved; dpi chosen so that pixel width maps to 120 mm.
    cursor_mm += 10.0;
    let img_w_mm = 120.0;
    let dpi = image.width as f32 / (img_w_mm / 25.4);
    let img_h_mm = image.height as f32 / dpi * 25.4;
    cursor_mm += img_h_mm;
    ops.push(Op::UseXobject {
        id: image_id,
        transform: XObjectTransform {
            translate_x: Some(x0),
            translate_y: Some(y_top(cursor_mm)),
            dpi: Some(dpi),
            ..Default::default()
        },
    });
    cursor_mm += 4.5;
    ops.push(Op::SetFillColor { col: grey.clone() });
    let caption = "Fotografia 1 · 24/09/2026 10:42".to_string();
    let cap_w = text_width(&regular, &caption, 9.0);
    ops.extend(text_block(&regular_id, 9.0, 11.0, Pt(x0.0 + (img_w_mm * MM_TO_PT - cap_w) / 2.0), y_top(cursor_mm), &[caption]));

    // Description heading + wrapped paragraph.
    cursor_mm += 10.0;
    ops.push(Op::SetFillColor { col: black.clone() });
    ops.extend(text_block(&bold_id, 12.0, 14.0, x0, y_top(cursor_mm), &["Descrição".to_string()]));
    cursor_mm += 5.5;
    let size = 10.5;
    let leading = size * 1.35;
    let lines = wrap(&regular, report.paragraph, size, content_w_pt);
    ops.extend(text_block(&regular_id, size, leading, x0, y_top(cursor_mm), &lines));
    cursor_mm += (lines.len() as f32 - 1.0) * leading / MM_TO_PT;

    // Status box.
    cursor_mm += 8.0;
    let box_h_mm = 7.0;
    ops.push(Op::SetFillColor { col: rgb(0xff, 0xf3, 0xcd) });
    ops.push(Op::DrawRectangle {
        rectangle: Rect {
            x: x0,
            y: y_top(cursor_mm + box_h_mm),
            width: pt(38.0),
            height: pt(box_h_mm),
            mode: Some(PaintMode::Fill),
            winding_order: None,
        },
    });
    ops.push(Op::SetFillColor { col: black.clone() });
    ops.extend(text_block(&bold_id, 9.0, 11.0, pt(MARGIN_MM + 2.0), y_top(cursor_mm + 4.7), &["Estado: Em aberto".to_string()]));

    // Footer.
    ops.push(Op::SetFillColor { col: grey });
    let footer = format!("Checkflat · {}", report.subtitle);
    ops.extend(text_block(&regular_id, 9.0, 11.0, x0, y_top(PAGE_H_MM - 12.0), &[footer]));
    let pageno = "1".to_string();
    let pn_w = text_width(&regular, &pageno, 9.0);
    ops.extend(text_block(&regular_id, 9.0, 11.0, Pt(pt(PAGE_W_MM - MARGIN_MM).0 - pn_w), y_top(PAGE_H_MM - 12.0), &[pageno]));

    let page = PdfPage::new(Mm(PAGE_W_MM), Mm(PAGE_H_MM), ops);
    let bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions { subset_fonts: true, ..Default::default() }, &mut warnings);
    Ok(bytes)
}
