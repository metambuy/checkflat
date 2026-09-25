//! PDF metadata for plan import, read in Rust with lopdf so import is one testable transaction
//! that behaves identically on Android and Windows. PDF.js remains the renderer.
use std::path::Path;

use lopdf::{Dictionary, Document, Object};
use serde::Serialize;

use crate::{CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfInfo {
    pub page_count: usize,
    /// Visible size of page 1 in PDF points, after applying /Rotate.
    pub width_pt: f64,
    pub height_pt: f64,
}

pub fn inspect_file(path: &Path) -> Result<PdfInfo> {
    let doc = Document::load(path).map_err(|e| CoreError::Unreadable(e.to_string()))?;
    inspect_doc(&doc)
}

pub fn inspect_bytes(bytes: &[u8]) -> Result<PdfInfo> {
    let doc = Document::load_mem(bytes).map_err(|e| CoreError::Unreadable(e.to_string()))?;
    inspect_doc(&doc)
}

fn inspect_doc(doc: &Document) -> Result<PdfInfo> {
    let pages = doc.get_pages();
    let page_count = pages.len();
    let (_, first) = pages.iter().next().ok_or_else(|| CoreError::Unreadable("no pages".into()))?;
    let page = doc.get_dictionary(*first).map_err(|e| CoreError::Unreadable(e.to_string()))?;

    let bx = inherited(doc, page, b"MediaBox")
        .or_else(|| inherited(doc, page, b"CropBox"))
        .ok_or_else(|| CoreError::Unreadable("page 1 has no MediaBox".into()))?;
    let rect = rect(doc, bx).ok_or_else(|| CoreError::Unreadable("invalid MediaBox".into()))?;
    let (mut w, mut h) = ((rect[2] - rect[0]).abs(), (rect[3] - rect[1]).abs());
    if w <= 0.0 || h <= 0.0 || !w.is_finite() || !h.is_finite() {
        return Err(CoreError::Unreadable("degenerate page size".into()));
    }
    let rotate = inherited(doc, page, b"Rotate")
        .and_then(|o| resolve(doc, o).as_i64().ok())
        .unwrap_or(0)
        .rem_euclid(360);
    if rotate == 90 || rotate == 270 {
        std::mem::swap(&mut w, &mut h);
    }
    Ok(PdfInfo { page_count, width_pt: w, height_pt: h })
}

/// Page attribute lookup honouring inheritance through the /Parent chain (MediaBox, Rotate, …).
fn inherited<'a>(doc: &'a Document, page: &'a Dictionary, key: &[u8]) -> Option<&'a Object> {
    let mut node = page;
    for _ in 0..64 {
        if let Ok(v) = node.get(key) {
            return Some(v);
        }
        let parent = node.get(b"Parent").ok()?.as_reference().ok()?;
        node = doc.get_dictionary(parent).ok()?;
    }
    None
}

fn resolve<'a>(doc: &'a Document, o: &'a Object) -> &'a Object {
    match o {
        Object::Reference(id) => doc.get_object(*id).unwrap_or(o),
        _ => o,
    }
}

fn rect(doc: &Document, o: &Object) -> Option<[f64; 4]> {
    let arr = resolve(doc, o).as_array().ok()?;
    if arr.len() != 4 {
        return None;
    }
    let mut out = [0.0; 4];
    for (i, v) in arr.iter().enumerate() {
        out[i] = number(resolve(doc, v))?;
    }
    Some(out)
}

fn number(o: &Object) -> Option<f64> {
    match o {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(f64::from(*r)),
        _ => None,
    }
}

/// Build a minimal PDF in memory (tests and tooling). `rotate` is applied to every page.
pub fn make_pdf(pages: &[(f32, f32)], rotate: i64) -> Vec<u8> {
    use lopdf::{dictionary, Stream};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();
    for (w, h) in pages {
        let content = Stream::new(dictionary! {}, b"0 0 m 10 10 l S".to_vec());
        let content_id = doc.add_object(content);
        let mut page = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), (*w).into(), (*h).into()],
            "Contents" => content_id,
        };
        if rotate != 0 {
            page.set("Rotate", rotate);
        }
        kids.push(Object::Reference(doc.add_object(page)));
    }
    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("in-memory save");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a1_single_page() {
        let info = inspect_bytes(&make_pdf(&[(2384.0, 1684.0)], 0)).unwrap();
        assert_eq!(info, PdfInfo { page_count: 1, width_pt: 2384.0, height_pt: 1684.0 });
    }

    #[test]
    fn rotate_swaps_dimensions() {
        let info = inspect_bytes(&make_pdf(&[(2384.0, 1684.0)], 90)).unwrap();
        assert_eq!((info.width_pt, info.height_pt), (1684.0, 2384.0));
        let info = inspect_bytes(&make_pdf(&[(2384.0, 1684.0)], 180)).unwrap();
        assert_eq!((info.width_pt, info.height_pt), (2384.0, 1684.0));
    }

    #[test]
    fn multi_page_counts_pages() {
        let info = inspect_bytes(&make_pdf(&[(595.0, 842.0), (595.0, 842.0)], 0)).unwrap();
        assert_eq!(info.page_count, 2);
    }

    #[test]
    fn garbage_is_unreadable() {
        assert!(matches!(inspect_bytes(b"hello"), Err(CoreError::Unreadable(_))));
    }
}
