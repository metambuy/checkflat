//! Observation refs (D-020): a per-project template renders project code + fraction + sequence
//! into the displayed ref. Nothing here touches the database; the stored facts are the project's
//! `code`, `ref_template`, `seq_scope` and each observation's `fraction` + `seq`.
//!
//! Template tokens: `{PROJ}` (project code), `{FRAC}` (fraction code), `{SEQ:n}` (sequence,
//! zero-padded to `n` digits, never truncated); everything else is literal text. A token that
//! renders empty disappears together with the literal before it; while nothing has been emitted
//! yet it takes the literal after it instead: `{PROJ}-{FRAC}-{SEQ:2}` gives `LAM-A-03`, `LAM-03`,
//! `A-03`, `03`. Prefix and suffix literals are always kept (`#{SEQ:2}` → `#03`).
use serde::{Deserialize, Serialize};

use crate::{CoreError, Result};

pub const DEFAULT_TEMPLATE: &str = "{PROJ}-{FRAC}-{SEQ:2}";
pub const MAX_TEMPLATE_LEN: usize = 64;
pub const MAX_CODE_LEN: usize = 16;
const MAX_SEQ_WIDTH: u8 = 6;

/// Whether sequence numbers run per project or per fraction. Stored as `project.seq_scope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Project,
    Fraction,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Project => "project",
            Scope::Fraction => "fraction",
        }
    }

    pub fn parse(s: &str) -> Result<Scope> {
        match s {
            "project" => Ok(Scope::Project),
            "fraction" => Ok(Scope::Fraction),
            other => Err(CoreError::Validation(format!("unknown sequence scope: {other}"))),
        }
    }

    /// The `observation.seq_key` for a fraction under this scope: the fraction code when numbers
    /// run per fraction, `''` when they run per project.
    pub fn seq_key(self, fraction: &str) -> &str {
        match self {
            Scope::Project => "",
            Scope::Fraction => fraction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Lit(String),
    Proj,
    Frac,
    Seq(u8),
}

/// A parsed, syntactically valid template (exactly one `{SEQ:n}`, each other token at most once).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pieces: Vec<Piece>,
    seq_width: u8,
}

impl Template {
    pub fn parse(src: &str) -> Result<Template> {
        let invalid = |reason: &str| CoreError::InvalidTemplate(reason.to_string());
        if src.trim().is_empty() {
            return Err(invalid("template is empty"));
        }
        if src.chars().count() > MAX_TEMPLATE_LEN {
            return Err(invalid("template is too long"));
        }
        let mut pieces = Vec::new();
        let mut lit = String::new();
        let mut rest = src;
        let (mut proj, mut frac, mut seq) = (0, 0, None);
        while let Some(open) = rest.find('{') {
            if rest[..open].contains('}') {
                return Err(invalid("unexpected '}'"));
            }
            lit.push_str(&rest[..open]);
            let after = &rest[open + 1..];
            let Some(close) = after.find('}') else { return Err(invalid("unclosed '{'")) };
            let token = &after[..close];
            let piece = match token {
                "PROJ" => {
                    proj += 1;
                    Piece::Proj
                }
                "FRAC" => {
                    frac += 1;
                    Piece::Frac
                }
                _ => match token.strip_prefix("SEQ:") {
                    Some(n) => {
                        let width: u8 = n.parse().map_err(|_| invalid("SEQ width must be a number"))?;
                        if !(1..=MAX_SEQ_WIDTH).contains(&width) {
                            return Err(invalid("SEQ width must be between 1 and 6"));
                        }
                        seq = Some(width);
                        Piece::Seq(width)
                    }
                    None => return Err(CoreError::InvalidTemplate(format!("unknown token {{{token}}}"))),
                },
            };
            if !lit.is_empty() {
                pieces.push(Piece::Lit(std::mem::take(&mut lit)));
            }
            pieces.push(piece);
            rest = &after[close + 1..];
        }
        if rest.contains('}') {
            return Err(invalid("unexpected '}'"));
        }
        lit.push_str(rest);
        if !lit.is_empty() {
            pieces.push(Piece::Lit(lit));
        }
        let Some(seq_width) = seq else { return Err(invalid("template needs exactly one {SEQ:n}")) };
        if pieces.iter().filter(|p| matches!(p, Piece::Seq(_))).count() > 1 {
            return Err(invalid("template needs exactly one {SEQ:n}"));
        }
        if proj > 1 || frac > 1 {
            return Err(invalid("{PROJ} and {FRAC} may appear only once"));
        }
        Ok(Template { pieces, seq_width })
    }

    /// Scope-specific rule: numbers per fraction must show the fraction.
    pub fn validate_for(&self, scope: Scope) -> Result<()> {
        if scope == Scope::Fraction && !self.pieces.contains(&Piece::Frac) {
            return Err(CoreError::InvalidTemplate("{FRAC} is required when numbers run per fraction".into()));
        }
        Ok(())
    }

    pub fn seq_width(&self) -> u8 {
        self.seq_width
    }

    pub fn pad_seq(&self, seq: i64) -> String {
        format!("{seq:0width$}", width = usize::from(self.seq_width))
    }

    pub fn render(&self, code: &str, fraction: &str, seq: i64) -> String {
        let mut out = String::new();
        let mut pending: Option<&str> = None; // literal waiting for the next non-empty token
        let mut emitted = false;
        let mut skip_next_lit = false;
        for piece in &self.pieces {
            let value = match piece {
                Piece::Lit(s) => {
                    if skip_next_lit {
                        skip_next_lit = false;
                    } else {
                        pending = Some(s);
                    }
                    continue;
                }
                Piece::Proj => code.to_string(),
                Piece::Frac => fraction.to_string(),
                Piece::Seq(_) => self.pad_seq(seq),
            };
            if value.is_empty() {
                if emitted {
                    pending = None; // drop the literal before this token
                } else {
                    skip_next_lit = true; // keep the prefix, drop the literal after
                }
            } else {
                if let Some(p) = pending.take() {
                    out.push_str(p);
                }
                out.push_str(&value);
                emitted = true;
            }
        }
        if let Some(p) = pending {
            out.push_str(p); // suffix literal (SEQ is never empty, so something was emitted)
        }
        out
    }
}

/// A project code or fraction code: trimmed; at most 16 characters; no braces, whitespace or
/// control characters. Empty is allowed (project code unset, no fraction) — callers decide.
pub fn validate_code(code: &str) -> Result<String> {
    let c = code.trim();
    if c.chars().count() > MAX_CODE_LEN {
        return Err(CoreError::Validation("code is too long".into()));
    }
    if c.chars().any(|ch| ch == '{' || ch == '}' || ch.is_whitespace() || ch.is_control()) {
        return Err(CoreError::Validation("code contains an invalid character".into()));
    }
    Ok(c.to_string())
}

/// Marker label on the plan: the sequence alone when numbers run per project, `FRAC-SEQ` when
/// they run per fraction (two pins on one plan never share a label).
pub fn marker(scope: Scope, template: &Template, fraction: &str, seq: i64) -> String {
    match scope {
        Scope::Project => seq.to_string(),
        Scope::Fraction => format!("{fraction}-{}", template.pad_seq(seq)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(t: &str, code: &str, frac: &str, seq: i64) -> String {
        Template::parse(t).unwrap().render(code, frac, seq)
    }

    #[test]
    fn renders_default_template_with_and_without_tokens() {
        let t = DEFAULT_TEMPLATE;
        assert_eq!(render(t, "LAM", "A", 3), "LAM-A-03");
        assert_eq!(render(t, "LAM", "", 3), "LAM-03");
        assert_eq!(render(t, "", "A", 3), "A-03");
        assert_eq!(render(t, "", "", 3), "03");
        assert_eq!(render(t, "LAM", "PC", 12), "LAM-PC-12");
    }

    #[test]
    fn other_separators_prefix_suffix_and_width() {
        assert_eq!(render("{PROJ}/{FRAC}/{SEQ:3}", "LAM", "A", 7), "LAM/A/007");
        assert_eq!(render("{PROJ}/{FRAC}/{SEQ:3}", "LAM", "", 7), "LAM/007");
        assert_eq!(render("#{SEQ:2}", "", "", 3), "#03");
        assert_eq!(render("#{PROJ}-{SEQ:2}", "", "", 3), "#03");
        assert_eq!(render("{SEQ:2}-{FRAC}", "", "", 3), "03");
        assert_eq!(render("{SEQ:2}-{FRAC}!", "", "A", 3), "03-A!");
        assert_eq!(render("{SEQ:2}-{FRAC}!", "", "", 3), "03!");
        assert_eq!(render("{FRAC}.{SEQ:1}", "", "A", 3), "A.3");
        assert_eq!(render("{SEQ:2}", "", "", 123), "123", "never truncated");
        assert_eq!(render("{SEQ:6}", "", "", 1), "000001");
    }

    #[test]
    fn rejects_invalid_templates() {
        for (t, why) in [
            ("", "empty"),
            ("LAM-{FRAC}", "no SEQ"),
            ("{SEQ:2}-{SEQ:2}", "two SEQ"),
            ("{SEQ}", "SEQ without width"),
            ("{SEQ:0}", "width 0"),
            ("{SEQ:7}", "width 7"),
            ("{SEQ:x}", "non-numeric width"),
            ("{PROJ}{PROJ}{SEQ:2}", "PROJ twice"),
            ("{NOPE}-{SEQ:2}", "unknown token"),
            ("{SEQ:2", "unclosed"),
            ("}{SEQ:2}", "stray close"),
            (&"x".repeat(70), "too long"),
        ] {
            assert!(matches!(Template::parse(t), Err(CoreError::InvalidTemplate(_))), "{why}: {t:?}");
        }
    }

    #[test]
    fn fraction_scope_requires_frac() {
        let t = Template::parse("{PROJ}-{SEQ:2}").unwrap();
        assert!(t.validate_for(Scope::Project).is_ok());
        assert!(matches!(t.validate_for(Scope::Fraction), Err(CoreError::InvalidTemplate(_))));
        assert!(Template::parse(DEFAULT_TEMPLATE).unwrap().validate_for(Scope::Fraction).is_ok());
    }

    #[test]
    fn codes_and_markers() {
        assert_eq!(validate_code("  pc ").unwrap(), "pc");
        assert_eq!(validate_code("").unwrap(), "");
        assert!(matches!(validate_code("a b"), Err(CoreError::Validation(_))));
        assert!(matches!(validate_code("{x}"), Err(CoreError::Validation(_))));
        assert!(matches!(validate_code(&"a".repeat(17)), Err(CoreError::Validation(_))));
        let t = Template::parse(DEFAULT_TEMPLATE).unwrap();
        assert_eq!(marker(Scope::Project, &t, "A", 3), "3");
        assert_eq!(marker(Scope::Fraction, &t, "A", 3), "A-03");
        assert_eq!(Scope::Project.seq_key("A"), "");
        assert_eq!(Scope::Fraction.seq_key("A"), "A");
    }
}
