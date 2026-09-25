//! UUID v7 identifiers: time-ordered, globally unique, safe for archive import and future sync.
use uuid::Uuid;

pub fn new_id() -> String {
    Uuid::now_v7().to_string()
}

/// True only for the canonical lowercase hyphenated form this app generates. `Uuid::parse_str`
/// also accepts `urn:uuid:…`, `{…}` and uppercase, which must not pass as path components.
pub fn is_id(s: &str) -> bool {
    Uuid::parse_str(s).map(|u| u.hyphenated().to_string() == s).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_v7_and_ordered() {
        let a = new_id();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = new_id();
        assert_eq!(Uuid::parse_str(&a).unwrap().get_version_num(), 7);
        assert!(a < b, "v7 ids sort by creation time");
        assert!(is_id(&a));
        assert!(!is_id("not-a-uuid"));
    }

    #[test]
    fn only_canonical_hyphenated_lowercase_form_is_an_id() {
        let a = new_id();
        assert!(!is_id(&format!("urn:uuid:{a}")));
        assert!(!is_id(&format!("{{{a}}}")));
        assert!(!is_id(&a.to_uppercase()));
        assert!(!is_id(&a.replace('-', "")));
        assert!(!is_id(""));
    }
}
