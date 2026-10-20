//! Photo annotations (Sprint 4, M6): the shapes drawn over a photo, stored as JSON in
//! `photo.annotation_json` and flattened onto the report copy by `checkflat-report::annotate`.
//! The photo file itself is never changed. Coordinates are fractions of the image's width and
//! height (0..1), so one set of shapes fits the stored ≤ 1600 px file, the thumbnails and the
//! 800 px report copy. One colour, one stroke width (a fraction of the long side), an arrow head
//! drawn as a chevron. The constants and the format are mirrored in `src/lib/annotate/model.ts`;
//! a report test pins the two against each other.
use serde::{Deserialize, Serialize};

use crate::{CoreError, Result};

pub const VERSION: u32 = 1;
pub const MAX_SHAPES: usize = 50;
pub const MAX_POINTS: usize = 2000;
/// Over all freehand shapes of one photo (bounds the JSON at ≈ 120 KB).
pub const MAX_TOTAL_POINTS: usize = 6000;
pub const MIN_POINTS: usize = 2;
/// Stroke width as a fraction of the image's long side: ≈ 6 px on an 800 px report photo (≈ 0.6 mm
/// printed at 80 mm), ≈ 3 CSS px on a 400 px-wide phone editor.
pub const STROKE_FRACTION: f64 = 0.0075;
/// Each leg of the arrow's chevron is this many stroke widths long …
pub const ARROW_HEAD: f64 = 4.0;
/// … at this angle from the shaft.
pub const ARROW_HEAD_ANGLE_DEG: f64 = 30.0;
/// `#E60000`.
pub const COLOUR: [u8; 3] = [0xE6, 0x00, 0x00];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Shape {
    /// Axis-aligned; `rx` is a fraction of the width, `ry` of the height.
    Ellipse { cx: f64, cy: f64, rx: f64, ry: f64 },
    /// Tail (1) → head (2).
    Arrow { x1: f64, y1: f64, x2: f64, y2: f64 },
    /// Polyline, [`MIN_POINTS`]..=[`MAX_POINTS`] points.
    Freehand { pts: Vec<[f64; 2]> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub v: u32,
    pub shapes: Vec<Shape>,
}

fn invalid(what: impl std::fmt::Display) -> CoreError {
    CoreError::Validation(format!("annotation: {what}"))
}

fn unit(name: &str, v: f64) -> Result<()> {
    if v.is_finite() && (0.0..=1.0).contains(&v) {
        Ok(())
    } else {
        Err(invalid(format!("{name} must be within 0..1, got {v}")))
    }
}

impl Annotation {
    pub fn new(shapes: Vec<Shape>) -> Self {
        Annotation { v: VERSION, shapes }
    }

    /// The format rules: version 1, at most [`MAX_SHAPES`], every coordinate a finite fraction in
    /// 0..1, ellipse radii positive, freehand point counts within bounds.
    pub fn validate(&self) -> Result<()> {
        if self.v != VERSION {
            return Err(invalid(format!("unsupported version {}", self.v)));
        }
        if self.shapes.len() > MAX_SHAPES {
            return Err(invalid(format!("more than {MAX_SHAPES} shapes")));
        }
        let mut total_points = 0;
        for s in &self.shapes {
            match s {
                Shape::Ellipse { cx, cy, rx, ry } => {
                    unit("cx", *cx)?;
                    unit("cy", *cy)?;
                    unit("rx", *rx)?;
                    unit("ry", *ry)?;
                    if *rx <= 0.0 || *ry <= 0.0 {
                        return Err(invalid("ellipse radii must be positive"));
                    }
                }
                Shape::Arrow { x1, y1, x2, y2 } => {
                    unit("x1", *x1)?;
                    unit("y1", *y1)?;
                    unit("x2", *x2)?;
                    unit("y2", *y2)?;
                }
                Shape::Freehand { pts } => {
                    if pts.len() < MIN_POINTS || pts.len() > MAX_POINTS {
                        return Err(invalid(format!("a freehand shape needs {MIN_POINTS}..={MAX_POINTS} points, got {}", pts.len())));
                    }
                    total_points += pts.len();
                    if total_points > MAX_TOTAL_POINTS {
                        return Err(invalid(format!("more than {MAX_TOTAL_POINTS} points in total")));
                    }
                    for [x, y] in pts {
                        unit("x", *x)?;
                        unit("y", *y)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Parses and validates the stored or submitted JSON.
    pub fn parse(json: &str) -> Result<Annotation> {
        let a: Annotation = serde_json::from_str(json).map_err(invalid)?;
        a.validate()?;
        Ok(a)
    }

    /// The canonical JSON for the column after validation; `None` when there is nothing drawn
    /// (the column stays `NULL`).
    pub fn to_json(&self) -> Result<Option<String>> {
        self.validate()?;
        if self.shapes.is_empty() {
            return Ok(None);
        }
        Ok(Some(serde_json::to_string(self).map_err(invalid)?))
    }
}

/// `Some(annotation)` for a stored column that parses, `None` for `NULL`; a stored value that no
/// longer parses is logged and treated as nothing drawn (a read never fails on it: the core
/// validated it on write, so this only happens to a hand-edited database).
pub fn parse_stored(json: Option<&str>) -> Option<Annotation> {
    let json = json?;
    match Annotation::parse(json) {
        Ok(a) if a.shapes.is_empty() => None,
        Ok(a) => Some(a),
        Err(e) => {
            log::warn!("stored annotation ignored: {e}");
            None
        }
    }
}

/// `Some(json)` when the annotation has shapes (validated), `None` otherwise.
pub fn to_column(annotation: Option<&Annotation>) -> Result<Option<String>> {
    Ok(annotation.map(Annotation::to_json).transpose()?.flatten())
}

/// Stroke width in pixels for an image of the given size.
pub fn stroke_px(width: u32, height: u32) -> f64 {
    STROKE_FRACTION * f64::from(width.max(height))
}

/// The ends of the two chevron legs of an arrow head, in the caller's units (pixels), for a shaft
/// from `tail` to `tip` and a leg of `len`; `None` when the arrow has no direction (tail = tip).
pub fn arrow_head(tail: (f64, f64), tip: (f64, f64), len: f64) -> Option<((f64, f64), (f64, f64))> {
    let (dx, dy) = (tail.0 - tip.0, tail.1 - tip.1);
    let d = dx.hypot(dy);
    if !(d > 0.0) {
        return None;
    }
    let (ux, uy) = (dx / d, dy / d);
    let (sin, cos) = ARROW_HEAD_ANGLE_DEG.to_radians().sin_cos();
    let leg = |s: f64| (tip.0 + len * (ux * cos - uy * s * sin), tip.1 + len * (uy * cos + ux * s * sin));
    Some((leg(1.0), leg(-1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANONICAL: &str = r#"{"v":1,"shapes":[{"t":"ellipse","cx":0.52,"cy":0.4,"rx":0.12,"ry":0.09},{"t":"arrow","x1":0.1,"y1":0.8,"x2":0.45,"y2":0.5},{"t":"freehand","pts":[[0.2,0.2],[0.21,0.23]]}]}"#;

    #[test]
    fn canonical_json_round_trips() {
        let a = Annotation::parse(CANONICAL).unwrap();
        assert_eq!(a.shapes.len(), 3);
        assert!(matches!(a.shapes[0], Shape::Ellipse { cx, .. } if cx == 0.52));
        assert!(matches!(a.shapes[1], Shape::Arrow { x2, .. } if x2 == 0.45));
        assert!(matches!(&a.shapes[2], Shape::Freehand { pts } if pts.len() == 2));
        assert_eq!(a.to_json().unwrap().as_deref(), Some(CANONICAL));
        assert_eq!(parse_stored(Some(CANONICAL)), Some(a));
    }

    #[test]
    fn nothing_drawn_is_null() {
        assert_eq!(Annotation::new(vec![]).to_json().unwrap(), None);
        assert_eq!(to_column(None).unwrap(), None);
        assert_eq!(to_column(Some(&Annotation::new(vec![]))).unwrap(), None);
        assert_eq!(parse_stored(None), None);
        assert_eq!(parse_stored(Some(r#"{"v":1,"shapes":[]}"#)), None);
        // A damaged column reads as nothing drawn instead of failing the read.
        assert_eq!(parse_stored(Some("{not json")), None);
        assert_eq!(parse_stored(Some(r#"{"v":1,"shapes":[{"t":"ellipse","cx":2,"cy":0,"rx":0.1,"ry":0.1}]}"#)), None);
    }

    #[test]
    fn invalid_annotations_are_refused() {
        let ellipse = |cx: f64, cy: f64, rx: f64, ry: f64| Shape::Ellipse { cx, cy, rx, ry };
        let bad: Vec<(&str, Annotation)> = vec![
            ("version", Annotation { v: 2, shapes: vec![] }),
            ("too many shapes", Annotation::new((0..51).map(|_| ellipse(0.5, 0.5, 0.1, 0.1)).collect())),
            ("out of range", Annotation::new(vec![ellipse(1.5, 0.5, 0.1, 0.1)])),
            ("negative", Annotation::new(vec![Shape::Arrow { x1: -0.1, y1: 0.0, x2: 0.5, y2: 0.5 }])),
            ("nan", Annotation::new(vec![Shape::Arrow { x1: f64::NAN, y1: 0.0, x2: 0.5, y2: 0.5 }])),
            ("zero radius", Annotation::new(vec![ellipse(0.5, 0.5, 0.0, 0.1)])),
            ("one point", Annotation::new(vec![Shape::Freehand { pts: vec![[0.1, 0.1]] }])),
            ("too many points", Annotation::new(vec![Shape::Freehand { pts: vec![[0.1, 0.1]; 2001] }])),
            ("too many in total", Annotation::new((0..4).map(|_| Shape::Freehand { pts: vec![[0.1, 0.1]; 2000] }).collect())),
            ("point out of range", Annotation::new(vec![Shape::Freehand { pts: vec![[0.1, 0.1], [0.1, 1.1]] }])),
        ];
        for (what, a) in bad {
            let err = a.validate().unwrap_err();
            assert!(matches!(err, CoreError::Validation(ref m) if m.starts_with("annotation: ")), "{what}: {err:?}");
            assert!(a.to_json().is_err(), "{what}");
        }
        for json in ["", "{}", "[]", r#"{"v":1}"#, r#"{"v":1,"shapes":[{"t":"square","x":0}]}"#, r#"{"v":1,"shapes":[{"t":"ellipse","cx":0.5}]}"#] {
            assert!(matches!(Annotation::parse(json), Err(CoreError::Validation(_))), "{json}");
        }
        // Limits are inclusive.
        assert!(Annotation::new((0..50).map(|_| ellipse(0.5, 0.5, 0.1, 0.1)).collect()).validate().is_ok());
        assert!(Annotation::new(vec![Shape::Freehand { pts: vec![[0.0, 0.0]; 2000] }, Shape::Freehand { pts: vec![[1.0, 1.0]; 2000] }]).validate().is_ok());
    }

    /// The same fixture is in `src/lib/annotate/geometry.test.ts`: a shaft from (0, 0) to (100, 0)
    /// with 10 px legs ends at (100 − 10·cos 30°, ±10·sin 30°).
    #[test]
    fn chevron_geometry_matches_the_frontend_fixture() {
        let ((ax, ay), (bx, by)) = arrow_head((0.0, 0.0), (100.0, 0.0), 10.0).unwrap();
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(close(ax, 91.33974596215562) && close(ay, -5.0), "{ax} {ay}");
        assert!(close(bx, 91.33974596215562) && close(by, 5.0), "{bx} {by}");
        // Pointing up (tail below the tip, y grows downwards): legs below the tip, either side.
        let ((ax, ay), (bx, by)) = arrow_head((50.0, 100.0), (50.0, 0.0), 10.0).unwrap();
        assert!(close(ay, 8.660254037844386) && close(by, 8.660254037844386));
        assert!(close(ax, 45.0) && close(bx, 55.0), "{ax} {bx}");
        assert_eq!(arrow_head((1.0, 1.0), (1.0, 1.0), 10.0), None);
        assert_eq!(stroke_px(800, 600), 6.0);
        assert_eq!(stroke_px(450, 800), 6.0);
    }
}
