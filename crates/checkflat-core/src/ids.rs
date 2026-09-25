//! UUID v7 identifiers: time-ordered, globally unique, safe for archive import and future sync.
use uuid::Uuid;

pub fn new_id() -> String {
    Uuid::now_v7().to_string()
}

pub fn is_id(s: &str) -> bool {
    Uuid::parse_str(s).is_ok()
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
}
