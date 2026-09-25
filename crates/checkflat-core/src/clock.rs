//! UTC ISO-8601 (RFC 3339) timestamps with millisecond precision, e.g. `2026-09-25T10:03:12.345Z`.
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub fn now_iso() -> String {
    format_iso(OffsetDateTime::now_utc())
}

pub fn format_iso(t: OffsetDateTime) -> String {
    let ms = t.millisecond();
    let t = t
        .replace_nanosecond(u32::from(ms) * 1_000_000)
        .expect("valid nanosecond");
    t.format(&Rfc3339).expect("RFC 3339 formatting cannot fail for UTC")
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
    fn iso_strings_sort_chronologically() {
        let a = "2026-09-25T10:03:12.345Z";
        let b = "2026-09-25T10:03:12.346Z";
        assert!(a < b);
        assert!(parse_iso(a).unwrap() < parse_iso(b).unwrap());
    }
}
