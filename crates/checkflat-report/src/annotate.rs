//! Photo preparation for the report. [`report_photo`] decodes the stored JPEG, brings it to print
//! size and, when the photo has an annotation, draws the shapes on that copy ([`flatten`]) before
//! encoding. The stored file is never changed, and the shapes go on the ≤ 800 px copy rather than
//! the full-size image, so nothing is added to the report's memory peak (Typst holds every decoded
//! image for the whole compile, D-024). Strokes are antialiased by `tiny-skia`, which the PDF
//! engine already brings in (typst-pdf → krilla-svg → resvg).
use std::io::Cursor;

use checkflat_core::annotation::{arrow_head, stroke_px, Annotation, Shape, ARROW_HEAD, COLOUR};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageEncoder, ImageReader, RgbImage};
use tiny_skia::{IntSize, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use crate::{ReportError, Result};

/// Photos print at 80 mm wide; 800 px is ~250 dpi there. Typst keeps every decoded image in
/// memory for the whole compile (D-024), so this also bounds the report's peak memory.
pub const REPORT_PHOTO_PX: u32 = 800;
pub const REPORT_JPEG_QUALITY: u8 = 80;

/// A photo ready for the PDF, with its pixel size (the template sizes the frame from it).
#[derive(Debug, Clone, PartialEq)]
pub struct ReportPhoto {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Downscales to at most [`REPORT_PHOTO_PX`] on the long side (JPEG q80) and draws the annotation
/// on that copy. A photo that is already small enough and has nothing drawn is embedded as stored
/// (it is a JPEG without EXIF, D-022).
pub fn report_photo(bytes: &[u8], annotation: Option<&Annotation>) -> Result<ReportPhoto> {
    let unreadable = |e: &dyn std::fmt::Display| ReportError::Image(e.to_string());
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| unreadable(&e))?;
    let img = reader.decode().map_err(|e| unreadable(&e))?;
    let drawn = annotation.filter(|a| !a.shapes.is_empty());
    let small = img.width().max(img.height()) <= REPORT_PHOTO_PX;
    if small && drawn.is_none() {
        return Ok(ReportPhoto { jpeg: bytes.to_vec(), width: img.width(), height: img.height() });
    }
    let mut rgb = if small {
        img.into_rgb8()
    } else {
        // The full decode goes before the drawing and the encode: one ≤ 800 px buffer from here on.
        let resized = img.resize(REPORT_PHOTO_PX, REPORT_PHOTO_PX, FilterType::Triangle).into_rgb8();
        drop(img);
        resized
    };
    if let Some(a) = drawn {
        flatten(&mut rgb, a)?;
    }
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, REPORT_JPEG_QUALITY)
        .write_image(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
        .map_err(|e| unreadable(&e))?;
    Ok(ReportPhoto { jpeg, width: rgb.width(), height: rgb.height() })
}

/// Draws the shapes onto `img` — the report copy, never the stored photo. Red, antialiased, one
/// stroke width from the image's long side, round caps and joins; the arrow head is a chevron.
/// The geometry (constants, head) is the core's, shared with the app's overlay.
pub fn flatten(img: &mut RgbImage, annotation: &Annotation) -> Result<()> {
    let (w, h) = (img.width(), img.height());
    let size = IntSize::from_wh(w, h).ok_or_else(|| ReportError::Image("empty image".into()))?;
    // Pixmaps are premultiplied RGBA; with every pixel opaque that is plain RGBA, so the copy in
    // and out is exact.
    let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
    for p in img.pixels() {
        rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
    }
    let mut pixmap = Pixmap::from_vec(rgba, size).ok_or_else(|| ReportError::Image("pixmap".into()))?;
    let width = stroke_px(w, h);
    let mut paint = Paint::default();
    paint.set_color_rgba8(COLOUR[0], COLOUR[1], COLOUR[2], 255);
    paint.anti_alias = true;
    let stroke = Stroke { width: width as f32, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() };
    let (fw, fh) = (f64::from(w), f64::from(h));
    for shape in &annotation.shapes {
        let path = match shape {
            Shape::Ellipse { cx, cy, rx, ry } => {
                Rect::from_xywh(((cx - rx) * fw) as f32, ((cy - ry) * fh) as f32, (2.0 * rx * fw) as f32, (2.0 * ry * fh) as f32).and_then(PathBuilder::from_oval)
            }
            Shape::Arrow { x1, y1, x2, y2 } => {
                let tail = (x1 * fw, y1 * fh);
                let tip = (x2 * fw, y2 * fh);
                let mut pb = PathBuilder::new();
                pb.move_to(tail.0 as f32, tail.1 as f32);
                pb.line_to(tip.0 as f32, tip.1 as f32);
                if let Some((a, b)) = arrow_head(tail, tip, ARROW_HEAD * width) {
                    pb.move_to(a.0 as f32, a.1 as f32);
                    pb.line_to(tip.0 as f32, tip.1 as f32);
                    pb.line_to(b.0 as f32, b.1 as f32);
                }
                pb.finish()
            }
            Shape::Freehand { pts } => {
                let mut pb = PathBuilder::new();
                for (i, [x, y]) in pts.iter().enumerate() {
                    let (px, py) = ((x * fw) as f32, (y * fh) as f32);
                    if i == 0 {
                        pb.move_to(px, py);
                    } else {
                        pb.line_to(px, py);
                    }
                }
                pb.finish()
            }
        };
        if let Some(path) = path {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
    let data = pixmap.take();
    for (p, src) in img.pixels_mut().zip(data.chunks_exact(4)) {
        p.0 = [src[0], src[1], src[2]];
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use checkflat_core::annotation::{ARROW_HEAD_ANGLE_DEG, MAX_POINTS, MAX_SHAPES, MAX_TOTAL_POINTS, STROKE_FRACTION};

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 7]));
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 85).encode_image(&img).unwrap();
        out
    }

    /// Uniform mid-grey, so a drawn pixel is unmistakable after the JPEG round trip.
    fn grey(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb([128, 128, 128]));
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 90).encode_image(&img).unwrap();
        out
    }

    fn decoded(jpeg: &[u8]) -> RgbImage {
        image::load_from_memory(jpeg).unwrap().to_rgb8()
    }
    fn is_red(p: &image::Rgb<u8>) -> bool {
        p[0] > 170 && p[1] < 90 && p[2] < 90
    }
    fn is_grey(p: &image::Rgb<u8>) -> bool {
        p.0.iter().all(|c| (i32::from(*c) - 128).abs() < 12)
    }
    fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Annotation {
        Annotation::new(vec![Shape::Ellipse { cx, cy, rx, ry }])
    }

    #[test]
    fn report_photo_caps_the_long_side_and_keeps_small_ones() {
        let big = report_photo(&jpeg(1600, 1200), None).unwrap();
        let dims = image::load_from_memory(&big.jpeg).unwrap();
        assert_eq!((dims.width(), dims.height()), (800, 600));
        assert_eq!((big.width, big.height), (800, 600));
        let tall = report_photo(&jpeg(900, 1600), None).unwrap();
        assert_eq!((tall.width, tall.height), (450, 800));
        let small = jpeg(640, 480);
        let kept = report_photo(&small, None).unwrap();
        assert_eq!((kept.jpeg, kept.width, kept.height), (small.clone(), 640, 480), "embedded as stored");
        // Nothing drawn (an empty annotation) is the same as no annotation.
        let kept = report_photo(&small, Some(&Annotation::new(vec![]))).unwrap();
        assert_eq!(kept.jpeg, small);
        assert!(matches!(report_photo(b"not an image", None), Err(ReportError::Image(_))));
    }

    #[test]
    fn a_small_photo_with_shapes_is_re_encoded_at_its_size() {
        let src = grey(640, 480);
        let out = report_photo(&src, Some(&ellipse(0.5, 0.5, 0.25, 0.25))).unwrap();
        assert_ne!(out.jpeg, src);
        assert_eq!((out.width, out.height), (640, 480));
        let img = decoded(&out.jpeg);
        assert!(is_red(img.get_pixel(480, 240)), "{:?}", img.get_pixel(480, 240));
        assert!(is_grey(img.get_pixel(320, 240)));
    }

    #[test]
    fn ellipse_arrow_and_freehand_are_drawn_red_on_the_downscaled_copy() {
        // 1600×1200 → 800×600; stroke 6 px; the ellipse's ring passes (600,300), (200,300), (400,150), (400,450).
        let src = grey(1600, 1200);
        let out = report_photo(&src, Some(&ellipse(0.5, 0.5, 0.25, 0.25))).unwrap();
        assert_eq!((out.width, out.height), (800, 600));
        let img = decoded(&out.jpeg);
        for (x, y) in [(600, 300), (200, 300), (400, 150), (400, 450)] {
            assert!(is_red(img.get_pixel(x, y)), "ring at {x},{y}: {:?}", img.get_pixel(x, y));
        }
        for (x, y) in [(400, 300), (20, 20), (780, 580), (400, 120)] {
            assert!(is_grey(img.get_pixel(x, y)), "untouched at {x},{y}: {:?}", img.get_pixel(x, y));
        }

        // Arrow (0.1,0.5) → (0.9,0.5): shaft on y = 300 from x = 80 to 720, chevron legs 24 px long
        // from the tip at 30°: their midpoints are near (710, 294) and (710, 306).
        let arrow = Annotation::new(vec![Shape::Arrow { x1: 0.1, y1: 0.5, x2: 0.9, y2: 0.5 }]);
        let img = decoded(&report_photo(&src, Some(&arrow)).unwrap().jpeg);
        for (x, y) in [(400, 300), (80, 300), (720, 300), (710, 294), (710, 306)] {
            assert!(is_red(img.get_pixel(x, y)), "arrow at {x},{y}: {:?}", img.get_pixel(x, y));
        }
        for (x, y) in [(720, 270), (760, 300), (690, 320), (60, 300)] {
            assert!(is_grey(img.get_pixel(x, y)), "untouched at {x},{y}: {:?}", img.get_pixel(x, y));
        }

        // Freehand through (160,120), (320,180), (480,120).
        let free = Annotation::new(vec![Shape::Freehand { pts: vec![[0.2, 0.2], [0.4, 0.3], [0.6, 0.2]] }]);
        let img = decoded(&report_photo(&src, Some(&free)).unwrap().jpeg);
        for (x, y) in [(160, 120), (320, 180), (480, 120), (240, 150), (400, 150)] {
            assert!(is_red(img.get_pixel(x, y)), "freehand at {x},{y}: {:?}", img.get_pixel(x, y));
        }
        assert!(is_grey(img.get_pixel(320, 120)), "inside the V, not on it");
    }

    #[test]
    fn strokes_are_antialiased() {
        // A diagonal shaft from (80,60) to (720,540) on 800×600: across it (along the normal
        // (−0.6, 0.8) through (400,300)) the red channel goes grey → red → grey through values in
        // between, which a hard-edged line would not have.
        let mut img = image::RgbImage::from_pixel(800, 600, image::Rgb([128, 128, 128]));
        flatten(&mut img, &Annotation::new(vec![Shape::Arrow { x1: 0.1, y1: 0.1, x2: 0.9, y2: 0.9 }])).unwrap();
        let reds: Vec<u8> = (-8..=8).map(|k| img.get_pixel((400.0 - 0.6 * k as f64).round() as u32, (300.0 + 0.8 * k as f64).round() as u32)[0]).collect();
        assert_eq!(reds[0], 128);
        assert_eq!(reds[16], 128);
        assert!(reds[8] > 220, "{reds:?}");
        assert!(reds.iter().any(|r| *r > 140 && *r < 215), "partial coverage at the edges: {reds:?}");
        assert_eq!(img.get_pixel(400, 300)[1], 0, "pure colour at the centre of the stroke");
    }

    /// The app's `model.ts` carries the same format and constants; keep them in step.
    #[test]
    fn frontend_model_mirrors_the_core_constants() {
        let ts = include_str!("../../../src/lib/annotate/model.ts");
        for line in [
            format!("STROKE_FRACTION = {STROKE_FRACTION};"),
            format!("ARROW_HEAD = {ARROW_HEAD};"),
            format!("ARROW_HEAD_ANGLE_DEG = {ARROW_HEAD_ANGLE_DEG};"),
            format!("COLOUR = \"#{:02x}{:02x}{:02x}\";", COLOUR[0], COLOUR[1], COLOUR[2]),
            format!("MAX_SHAPES = {MAX_SHAPES};"),
            format!("MAX_POINTS = {MAX_POINTS};"),
            format!("MAX_TOTAL_POINTS = {MAX_TOTAL_POINTS};"),
            "ANNOTATION_VERSION = 1;".to_string(),
        ] {
            assert!(ts.contains(&line), "src/lib/annotate/model.ts must contain `{line}`");
        }
    }
}
