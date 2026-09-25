//! UTC ISO-8601 (RFC 3339) timestamps with millisecond precision, e.g. `2026-09-25T10:03:12.345Z`.
use time::format_description::well_known::Rfc3339;
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

/// Fixed-width (24 chars) so strings sort chronologically. The well-known `Rfc3339` formatter
/// trims trailing zeros of the fraction (`.41Z`), which would break lexicographic ordering.
const ISO_MS: &[BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");

pub fn now_iso() -> String {
    format_iso(OffsetDateTime::now_utc())
}

pub fn format_iso(t: OffsetDateTime) -> String {
    t.to_offset(UtcOffset::UTC)
        .format(ISO_MS)
        .expect("fixed format cannot fail")
}

/// Filesystem-safe variant of [`now_iso`] (no `:` or `.`), e.g. `20260925T100312345Z`.
pub fn now_compact() -> String {
    now_iso().chars().filter(|c| !matches!(c, ':' | '-' | '.')).collect()
}

pub fn parse_iso(s: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(s, &Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_iso_round_trips_with_millis_and_z() {
        let s = now_iso();
        assert!(s.ends_with('Z'), "{s}");
        assert_eq!(s.len(), "2026-09-25T10:03:12.345Z".len(), "{s}");
        let parsed = parse_iso(&s).expect("parses back");
        assert_eq!(format_iso(parsed), s);
    }

    #[test]
    fn fraction_keeps_trailing_zeros() {
        let t = parse_iso("2026-09-25T10:44:04.400Z").unwrap();
        assert_eq!(format_iso(t), "2026-09-25T10:44:04.400Z");
        let t0 = parse_iso("2026-09-25T10:44:04Z").unwrap();
        assert_eq!(format_iso(t0), "2026-09-25T10:44:04.000Z");
        // Sub-millisecond precision is truncated, never rounded up into the next second.
        let t9 = parse_iso("2026-09-25T10:44:04.999999Z").unwrap();
        assert_eq!(format_iso(t9), "2026-09-25T10:44:04.999Z");
    }

    #[test]
    fn iso_strings_sort_chronologically() {
        let a = "2026-09-25T10:03:12.345Z";
        let b = "2026-09-25T10:03:12.346Z";
        assert!(a < b);
        assert!(parse_iso(a).unwrap() < parse_iso(b).unwrap());
    }
}
