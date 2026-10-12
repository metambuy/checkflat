//! Photo preparation for the report. Every photo goes through [`flatten`] first — the hook where
//! Sprint 4 will draw `photo.annotation_json` (ellipse, arrow, freehand, red) onto the image —
//! and then through [`report_photo`], which brings it to print size.
use std::borrow::Cow;
use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageEncoder, ImageReader};

use crate::{ReportError, Result};

/// Photos print at 80 mm wide; 800 px is ~250 dpi there. Typst keeps every decoded image in
/// memory for the whole compile (D-024), so this also bounds the report's peak memory.
pub const REPORT_PHOTO_PX: u32 = 800;
pub const REPORT_JPEG_QUALITY: u8 = 80;

/// Flattens the annotation onto the stored JPEG. **Sprint 5: returns the photo unchanged**; Sprint 4
/// implements the drawing here, and nothing else in the report needs to change for it.
pub fn flatten<'a>(jpeg: &'a [u8], annotation_json: Option<&str>) -> Result<Cow<'a, [u8]>> {
    let _ = annotation_json; // Sprint 4
    Ok(Cow::Borrowed(jpeg))
}

/// Downscales to at most [`REPORT_PHOTO_PX`] on the long side (JPEG q80). A photo that is already
/// small enough is embedded as stored (it is a JPEG without EXIF, D-022).
pub fn report_photo(bytes: &[u8]) -> Result<Vec<u8>> {
    let unreadable = |e: &dyn std::fmt::Display| ReportError::Image(e.to_string());
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| unreadable(&e))?;
    let img = reader.decode().map_err(|e| unreadable(&e))?;
    if img.width().max(img.height()) <= REPORT_PHOTO_PX {
        return Ok(bytes.to_vec());
    }
    let rgb = img.resize(REPORT_PHOTO_PX, REPORT_PHOTO_PX, FilterType::Triangle).to_rgb8();
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, REPORT_JPEG_QUALITY)
        .write_image(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
        .map_err(|e| unreadable(&e))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 7]));
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 85).encode_image(&img).unwrap();
        out
    }

    #[test]
    fn flatten_returns_the_photo_unchanged_until_sprint_4() {
        let src = jpeg(40, 30);
        assert_eq!(flatten(&src, None).unwrap().as_ref(), src.as_slice());
        let drawn = flatten(&src, Some(r#"{"shapes":[{"type":"ellipse","x":0.2,"y":0.3,"rx":0.1,"ry":0.1}]}"#)).unwrap();
        assert_eq!(drawn.as_ref(), src.as_slice());
        assert!(matches!(drawn, Cow::Borrowed(_)), "no copy is made");
    }

    #[test]
    fn report_photo_caps_the_long_side_and_keeps_small_ones() {
        let big = report_photo(&jpeg(1600, 1200)).unwrap();
        let dims = image::load_from_memory(&big).unwrap();
        assert_eq!((dims.width(), dims.height()), (800, 600));
        let tall = image::load_from_memory(&report_photo(&jpeg(900, 1600)).unwrap()).unwrap();
        assert_eq!((tall.width(), tall.height()), (450, 800));
        let small = jpeg(640, 480);
        assert_eq!(report_photo(&small).unwrap(), small, "embedded as stored");
        assert!(matches!(report_photo(b"not an image"), Err(ReportError::Image(_))));
    }
}
