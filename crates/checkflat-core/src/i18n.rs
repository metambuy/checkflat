//! EN/PT strings shared by the UI (`src/lib/i18n.ts`) and Rust (report generation later).
//! Both read the same `i18n/<lang>.json` files at the repo root.
use std::collections::HashMap;
use std::sync::OnceLock;

pub const EN_JSON: &str = include_str!("../../../i18n/en.json");
pub const PT_JSON: &str = include_str!("../../../i18n/pt.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Pt,
}

impl Lang {
    pub fn parse(s: &str) -> Lang {
        if s.to_ascii_lowercase().starts_with("pt") {
            Lang::Pt
        } else {
            Lang::En
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Pt => "pt",
        }
    }
}

fn table(lang: Lang) -> &'static HashMap<String, String> {
    static EN: OnceLock<HashMap<String, String>> = OnceLock::new();
    static PT: OnceLock<HashMap<String, String>> = OnceLock::new();
    let (cell, src) = match lang {
        Lang::En => (&EN, EN_JSON),
        Lang::Pt => (&PT, PT_JSON),
    };
    cell.get_or_init(|| serde_json::from_str(src).expect("i18n json is valid"))
}

/// Translate `key` with `{name}` placeholders replaced from `params`; falls back to English,
/// then to the key itself (so a missing string is visible, never a crash).
pub fn t(lang: Lang, key: &str, params: &[(&str, &str)]) -> String {
    let raw = table(lang)
        .get(key)
        .or_else(|| table(Lang::En).get(key))
        .cloned()
        .unwrap_or_else(|| key.to_string());
    params.iter().fold(raw, |acc, (k, v)| acc.replace(&format!("{{{k}}}"), v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn en_and_pt_have_identical_keys() {
        let en: HashMap<String, String> = serde_json::from_str(EN_JSON).unwrap();
        let pt: HashMap<String, String> = serde_json::from_str(PT_JSON).unwrap();
        let mut only_en: Vec<_> = en.keys().filter(|k| !pt.contains_key(*k)).collect();
        let mut only_pt: Vec<_> = pt.keys().filter(|k| !en.contains_key(*k)).collect();
        only_en.sort();
        only_pt.sort();
        assert!(only_en.is_empty() && only_pt.is_empty(), "missing in pt: {only_en:?}; missing in en: {only_pt:?}");
        fn placeholders(s: &str) -> Vec<String> {
            let mut v: Vec<String> = s.split('{').skip(1).filter_map(|x| x.split('}').next()).map(str::to_string).collect();
            v.sort();
            v
        }
        for (k, v) in &en {
            assert_eq!(placeholders(v), placeholders(&pt[k]), "placeholders differ for {k}");
        }
    }

    #[test]
    fn translate_with_params_and_fallbacks() {
        assert_eq!(t(Lang::Pt, "import.multi_page", &[("pages", "3")]), "Este PDF tem 3 páginas. O Checkflat precisa de uma planta por ficheiro.");
        assert_eq!(t(Lang::En, "nope.missing", &[]), "nope.missing");
        assert_eq!(Lang::parse("pt-PT"), Lang::Pt);
        assert_eq!(Lang::parse("en-GB"), Lang::En);
    }
}
