//! Photo preparation for the report. [`report_photo`] decodes the stored JPEG, brings it to print
//! size and, when the photo has an annotation, draws the shapes on that copy ([`flatten`]) before
//! encoding. The stored file is never changed, and the shapes go on the ≤ 640 px copy rather than
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

/// Photos print at 80 mm wide; 640 px is ~200 dpi there. Typst keeps every decoded image in
/// memory for the whole compile (D-024), so this also bounds the report's peak memory: 640 px
/// instead of 800 px is 0.52 MB less per landscape photo, ≈ 78 MB at 50 × 3 (D-029).
pub const REPORT_PHOTO_PX: u32 = 640;
pub const REPORT_JPEG_QUALITY: u8 = 75;

/// A photo ready for the PDF, with its pixel size (the template sizes the frame from it).
#[derive(Debug, Clone, PartialEq)]
pub struct ReportPhoto {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Downscales to at most [`REPORT_PHOTO_PX`] on the long side (JPEG [`REPORT_JPEG_QUALITY`]) and draws the annotation
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
        // The full decode goes before the drawing and the encode: one report-size buffer from here on.
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
        assert_eq!((dims.width(), dims.height()), (640, 480));
        assert_eq!((big.width, big.height), (640, 480));
        let tall = report_photo(&jpeg(900, 1600), None).unwrap();
        assert_eq!((tall.width, tall.height), (360, 640));
        let at_cap = jpeg(640, 480);
        assert_eq!(report_photo(&at_cap, None).unwrap().jpeg, at_cap, "at the cap: embedded as stored");
        let small = jpeg(480, 360);
        let kept = report_photo(&small, None).unwrap();
        assert_eq!((kept.jpeg, kept.width, kept.height), (small.clone(), 480, 360), "embedded as stored");
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
        // 1600×1200 → 640×480 (the cap); stroke 4.8 px. Probes are fractions of the copy, moved by
        // whole strokes where a shape's own geometry is in pixels (the chevron).
        let src = grey(1600, 1200);
        let out = report_photo(&src, Some(&ellipse(0.5, 0.5, 0.25, 0.25))).unwrap();
        assert_eq!((out.width, out.height), (640, 480));
        let (fw, fh) = (f64::from(out.width), f64::from(out.height));
        let stroke = stroke_px(out.width, out.height);
        let at = |fx: f64, fy: f64, dx: f64, dy: f64| ((fx * fw + dx).round() as u32, (fy * fh + dy).round() as u32);
        let img = decoded(&out.jpeg);
        for (fx, fy) in [(0.75, 0.5), (0.25, 0.5), (0.5, 0.25), (0.5, 0.75)] {
            let (x, y) = at(fx, fy, 0.0, 0.0);
            assert!(is_red(img.get_pixel(x, y)), "ring at {x},{y}: {:?}", img.get_pixel(x, y));
        }
        for (fx, fy) in [(0.5, 0.5), (0.025, 0.033), (0.975, 0.967), (0.5, 0.2)] {
            let (x, y) = at(fx, fy, 0.0, 0.0);
            assert!(is_grey(img.get_pixel(x, y)), "untouched at {x},{y}: {:?}", img.get_pixel(x, y));
        }

        // Arrow (0.1,0.5) → (0.9,0.5): the shaft along the middle, chevron legs 4 strokes long from
        // the tip at 30°: their midpoints sit (−√3·stroke, ±stroke) from the tip.
        let arrow = Annotation::new(vec![Shape::Arrow { x1: 0.1, y1: 0.5, x2: 0.9, y2: 0.5 }]);
        let img = decoded(&report_photo(&src, Some(&arrow)).unwrap().jpeg);
        let leg = -stroke * 3f64.sqrt();
        for (fx, dx, dy) in [(0.5, 0.0, 0.0), (0.1, 0.0, 0.0), (0.9, 0.0, 0.0), (0.9, leg, -stroke), (0.9, leg, stroke)] {
            let (x, y) = at(fx, 0.5, dx, dy);
            assert!(is_red(img.get_pixel(x, y)), "arrow at {x},{y}: {:?}", img.get_pixel(x, y));
        }
        for (fx, fy) in [(0.9, 0.45), (0.95, 0.5), (0.8625, 0.533), (0.075, 0.5)] {
            let (x, y) = at(fx, fy, 0.0, 0.0);
            assert!(is_grey(img.get_pixel(x, y)), "untouched at {x},{y}: {:?}", img.get_pixel(x, y));
        }

        // Freehand "V" through (0.2,0.2), (0.4,0.3), (0.6,0.2).
        let free = Annotation::new(vec![Shape::Freehand { pts: vec![[0.2, 0.2], [0.4, 0.3], [0.6, 0.2]] }]);
        let img = decoded(&report_photo(&src, Some(&free)).unwrap().jpeg);
        for (fx, fy) in [(0.2, 0.2), (0.4, 0.3), (0.6, 0.2), (0.3, 0.25), (0.5, 0.25)] {
            let (x, y) = at(fx, fy, 0.0, 0.0);
            assert!(is_red(img.get_pixel(x, y)), "freehand at {x},{y}: {:?}", img.get_pixel(x, y));
        }
        let (x, y) = at(0.4, 0.2, 0.0, 0.0);
        assert!(is_grey(img.get_pixel(x, y)), "inside the V, not on it");
    }

    /// Portrait photos: the fractions apply to the photo's own width and height, so an ellipse with
    /// rx = ry = 0.25 is a tall oval (not a circle of radius w/4), an arrow down the middle runs
    /// along the long side, and the stroke follows the long side (the height). A 1200×1600 photo
    /// (downscaled to 480×640) and a 480×640 one (at the cap: kept at its size, the small path).
    #[test]
    fn portrait_photos_get_their_shapes_where_the_app_shows_them() {
        for (src, (w, h)) in [(grey(1200, 1600), (480u32, 640u32)), (grey(480, 640), (480, 640))] {
            let (fw, fh) = (f64::from(w), f64::from(h));
            let stroke = stroke_px(w, h);
            assert_eq!(stroke, STROKE_FRACTION * fh, "the long side is the height");
            // The pixel at fractions (fx, fy) of the photo, moved by (dx, dy) pixels.
            let pix = |img: &RgbImage, fx: f64, fy: f64, dx: f64, dy: f64| *img.get_pixel((fx * fw + dx).round() as u32, (fy * fh + dy).round() as u32);
            let red_at = |img: &RgbImage, what: &str, fx: f64, fy: f64, dx: f64, dy: f64| {
                let p = pix(img, fx, fy, dx, dy);
                assert!(is_red(&p), "{what} at ({fx}, {fy}) + ({dx:.1}, {dy:.1}) of {w}x{h}: {p:?}");
            };
            let grey_at = |img: &RgbImage, what: &str, fx: f64, fy: f64, dx: f64, dy: f64| {
                let p = pix(img, fx, fy, dx, dy);
                assert!(is_grey(&p), "{what} at ({fx}, {fy}) + ({dx:.1}, {dy:.1}) of {w}x{h}: {p:?}");
            };

            // Ellipse: the ring is w/2 wide and h/2 tall; a circle of radius w/4 would have its top
            // and bottom nearer the centre, exactly where the oval is empty.
            let out = report_photo(&src, Some(&ellipse(0.5, 0.5, 0.25, 0.25))).unwrap();
            assert_eq!((out.width, out.height), (w, h));
            let img = decoded(&out.jpeg);
            assert_eq!(img.dimensions(), (w, h));
            for (fx, fy) in [(0.75, 0.5), (0.25, 0.5), (0.5, 0.25), (0.5, 0.75)] {
                red_at(&img, "ring", fx, fy, 0.0, 0.0);
            }
            for (what, dy) in [("a circle's top", -0.25 * fw), ("a circle's bottom", 0.25 * fw), ("centre", 0.0)] {
                grey_at(&img, what, 0.5, 0.5, 0.0, dy);
            }
            grey_at(&img, "corner", 0.02, 0.02, 0.0, 0.0);
            grey_at(&img, "corner", 0.98, 0.98, 0.0, 0.0);

            // Arrow from (0.5, 0.9) up to (0.5, 0.1): shaft on x = w/2; the chevron legs run from the
            // tip down and out, 4 strokes long at 30°, so their midpoints sit (±stroke, 1.73·stroke)
            // from the tip.
            let arrow = Annotation::new(vec![Shape::Arrow { x1: 0.5, y1: 0.9, x2: 0.5, y2: 0.1 }]);
            let img = decoded(&report_photo(&src, Some(&arrow)).unwrap().jpeg);
            assert_eq!(img.dimensions(), (w, h));
            let leg = stroke * 3f64.sqrt();
            for (what, fy, dy) in [("tail", 0.9, 0.0), ("middle", 0.5, 0.0), ("shaft near the tip", 0.1, 2.0 * stroke)] {
                red_at(&img, what, 0.5, fy, 0.0, dy);
            }
            red_at(&img, "left chevron leg", 0.5, 0.1, -stroke, leg);
            red_at(&img, "right chevron leg", 0.5, 0.1, stroke, leg);
            for (what, fx, fy) in [("left of the shaft", 0.25, 0.5), ("right of the shaft", 0.75, 0.5), ("above the tip", 0.5, 0.04), ("below the tail", 0.5, 0.97)] {
                grey_at(&img, what, fx, fy, 0.0, 0.0);
            }

            // Freehand ">" through (0.3, 0.2), (0.7, 0.5), (0.3, 0.8).
            let free = Annotation::new(vec![Shape::Freehand { pts: vec![[0.3, 0.2], [0.7, 0.5], [0.3, 0.8]] }]);
            let img = decoded(&report_photo(&src, Some(&free)).unwrap().jpeg);
            for (fx, fy) in [(0.3, 0.2), (0.7, 0.5), (0.3, 0.8), (0.5, 0.35), (0.5, 0.65)] {
                red_at(&img, "freehand", fx, fy, 0.0, 0.0);
            }
            grey_at(&img, "between the arms", 0.4, 0.5, 0.0, 0.0);
            grey_at(&img, "outside the point", 0.85, 0.5, 0.0, 0.0);
        }
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
