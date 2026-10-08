//! Photo import (D-022). `stage` takes the picked or captured image once, applies its EXIF
//! orientation, downsizes it to at most [`MAX_LONG_SIDE`] px on the long side and writes a JPEG to
//! `tmp/<token>.jpg` together with the capture time; the observation save then moves it into
//! `projects/<project_id>/photos/<photo_id>.jpg` (`repo::photos::attach`). The stored JPEG carries
//! no EXIF: the orientation is already applied (a viewer must not rotate it again) and GPS is gone.
//!
//! Decoders: JPEG, PNG and WebP. HEIC/HEIF is not readable here; on Android the plugin converts it
//! to JPEG before the file reaches this module.
use std::io::{Cursor, Read, Write};
use std::path::Path;

use exif::{In, Tag, Value};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageReader};
use serde::{Deserialize, Serialize};
use time::macros::format_description;
use time::{PrimitiveDateTime, UtcOffset};

use crate::paths::{staging_photo, TMP_DIR};
use crate::{clock, ids, CoreError, Result};

pub const MAX_LONG_SIDE: u32 = 1600;
pub const JPEG_QUALITY: u8 = 85;
/// Larger sources are refused before decoding (a 50 MP phone JPEG is ~15 MB).
pub const MAX_SOURCE_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedPhoto {
    pub token: String,
    /// UTC, RFC 3339: EXIF capture time, or now when the file has none.
    pub taken_at: String,
}

/// A staged photo handed back by the UI when the observation is saved.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoInput {
    pub token: String,
    pub taken_at: String,
}

#[derive(Debug)]
pub struct Processed {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub taken_at: Option<String>,
}

fn unreadable(e: impl std::fmt::Display) -> CoreError {
    CoreError::UnreadableImage(e.to_string())
}

/// `ftyp` box with a HEIC/HEIF major brand.
fn is_heif(bytes: &[u8]) -> bool {
    bytes.len() >= 12
        && &bytes[4..8] == b"ftyp"
        && matches!(&bytes[8..12], b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" | b"mif1" | b"msf1")
}

/// Decode, apply EXIF orientation, downsize, re-encode. `utc_offset_min` (minutes east of UTC) is
/// the zone assumed for an EXIF time that states none (EXIF `DateTimeOriginal` is local time).
pub fn process(bytes: &[u8], utc_offset_min: i32) -> Result<Processed> {
    if is_heif(bytes) {
        return Err(CoreError::UnsupportedImage);
    }
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(unreadable)?;
    let mut decoder = reader.into_decoder().map_err(unreadable)?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let taken_at = decoder.exif_metadata().ok().flatten().and_then(|c| exif_taken_at(&c, utc_offset_min));
    let mut img = DynamicImage::from_decoder(decoder).map_err(unreadable)?;
    img.apply_orientation(orientation);
    if img.width().max(img.height()) > MAX_LONG_SIDE {
        // Triangle: close to Lanczos for photos at a fraction of the cost on a phone CPU.
        img = img.resize(MAX_LONG_SIDE, MAX_LONG_SIDE, FilterType::Triangle);
    }
    let rgb = img.to_rgb8(); // JPEG has no alpha
    let (width, height) = (rgb.width(), rgb.height());
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY)
        .write_image(rgb.as_raw(), width, height, ExtendedColorType::Rgb8)
        .map_err(unreadable)?;
    Ok(Processed { jpeg, width, height, taken_at })
}

/// Read `source` (at most [`MAX_SOURCE_BYTES`]), process it and write `tmp/<token>.jpg`
/// (temp file + fsync + rename). Nothing is left behind on error.
pub fn stage(data_dir: &Path, source: impl Read, utc_offset_min: i32) -> Result<StagedPhoto> {
    let mut bytes = Vec::new();
    source.take(MAX_SOURCE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(unreadable("file is larger than 50 MB"));
    }
    let processed = process(&bytes, utc_offset_min)?;
    drop(bytes);

    std::fs::create_dir_all(data_dir.join(TMP_DIR))?;
    let token = ids::new_id();
    let dest = staging_photo(&token).resolve(data_dir);
    let part = dest.with_extension("jpg.part");
    let written = (|| -> std::io::Result<()> {
        let mut f = std::fs::File::create(&part)?;
        f.write_all(&processed.jpeg)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&part, &dest)
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&part);
        return Err(e.into());
    }
    Ok(StagedPhoto { token, taken_at: processed.taken_at.unwrap_or_else(clock::now_iso) })
}

/// Absolute location of a staged photo (for the asset protocol).
pub fn staged_path(data_dir: &Path, token: &str) -> Result<std::path::PathBuf> {
    if !ids::is_id(token) {
        return Err(CoreError::Validation("invalid token".into()));
    }
    Ok(staging_photo(token).resolve(data_dir))
}

/// Which of these tokens still have their staged file (the 24 h `tmp/` rule may have removed some).
/// Anything that is not a token is reported as missing.
pub fn staged_present(data_dir: &Path, tokens: &[String]) -> Vec<String> {
    tokens
        .iter()
        .filter(|t| staged_path(data_dir, t).map(|p| p.is_file()).unwrap_or(false))
        .cloned()
        .collect()
}

/// Discard a staged photo (removed from the sheet, or the sheet was cancelled). A missing file is fine.
pub fn discard_staged(data_dir: &Path, token: &str) -> Result<()> {
    match std::fs::remove_file(staged_path(data_dir, token)?) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// UTC ISO capture time from a raw EXIF chunk (TIFF structure, optionally with the `Exif\0\0`
/// prefix). `DateTimeOriginal`, else `DateTimeDigitized`, else `DateTime`; the zone is
/// `OffsetTimeOriginal`/`OffsetTime` when present, else `utc_offset_min`.
pub fn exif_taken_at(chunk: &[u8], utc_offset_min: i32) -> Option<String> {
    let raw = chunk.strip_prefix(b"Exif\0\0").unwrap_or(chunk).to_vec();
    let exif = exif::Reader::new().read_raw(raw).ok()?;
    let ascii = |tag: Tag| -> Option<String> {
        match &exif.get_field(tag, In::PRIMARY)?.value {
            Value::Ascii(v) => {
                let s = std::str::from_utf8(v.first()?).ok()?;
                let s = s.trim_matches(|c: char| c == '\0' || c.is_whitespace());
                (!s.is_empty()).then(|| s.to_string())
            }
            _ => None,
        }
    };
    let local = ascii(Tag::DateTimeOriginal).or_else(|| ascii(Tag::DateTimeDigitized)).or_else(|| ascii(Tag::DateTime))?;
    let offset = ascii(Tag::OffsetTimeOriginal).or_else(|| ascii(Tag::OffsetTime)).and_then(|s| parse_offset_secs(&s));
    exif_local_to_iso(&local, offset.unwrap_or(utc_offset_min.saturating_mul(60)))
}

/// `+02:00` / `-03:30` → seconds east of UTC.
fn parse_offset_secs(s: &str) -> Option<i32> {
    let sign = match s.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let (h, m) = s[1..].split_once(':')?;
    let (h, m): (i32, i32) = (h.parse().ok()?, m.parse().ok()?);
    (h <= 14 && m < 60).then_some(sign * (h * 3600 + m * 60))
}

/// EXIF local time `2026:10:05 14:03:09` at `offset_secs` east of UTC → UTC ISO string. Zero or
/// pre-2000 dates (cameras without a clock write `0000:00:00 …`) are rejected.
pub fn exif_local_to_iso(local: &str, offset_secs: i32) -> Option<String> {
    let t = PrimitiveDateTime::parse(local, format_description!("[year]:[month]:[day] [hour]:[minute]:[second]")).ok()?;
    if t.year() < 2000 {
        return None;
    }
    let offset = UtcOffset::from_whole_seconds(offset_secs).ok()?;
    Some(clock::format_iso(t.assume_offset(offset)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exif_time_to_utc() {
        assert_eq!(exif_local_to_iso("2026:10:05 14:03:09", 2 * 3600).as_deref(), Some("2026-10-05T12:03:09.000Z"));
        assert_eq!(exif_local_to_iso("2026:10:05 01:00:00", -3 * 3600).as_deref(), Some("2026-10-05T04:00:00.000Z"));
        assert_eq!(exif_local_to_iso("0000:00:00 00:00:00", 0), None);
        assert_eq!(exif_local_to_iso("garbage", 0), None);
    }

    #[test]
    fn offsets() {
        assert_eq!(parse_offset_secs("+02:00"), Some(7200));
        assert_eq!(parse_offset_secs("-03:30"), Some(-12600));
        assert_eq!(parse_offset_secs("Z"), None);
        assert_eq!(parse_offset_secs("+25:00"), None);
    }

    #[test]
    fn heif_brands_are_recognised() {
        let mut b = vec![0, 0, 0, 24];
        b.extend_from_slice(b"ftypheic");
        b.extend_from_slice(&[0; 8]);
        assert!(is_heif(&b));
        assert!(!is_heif(b"\xff\xd8\xff\xe0\x00\x10JFIF\0\x01\x01\x00"));
    }
}
