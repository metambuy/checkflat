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

use crate::error::{CodeProblem, TemplateProblem};
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
        let invalid = CoreError::InvalidTemplate;
        if src.trim().is_empty() {
            return Err(invalid(TemplateProblem::Empty));
        }
        if src.chars().count() > MAX_TEMPLATE_LEN {
            return Err(invalid(TemplateProblem::TooLong { max: MAX_TEMPLATE_LEN }));
        }
        let mut pieces = Vec::new();
        let mut lit = String::new();
        let mut rest = src;
        let (mut proj, mut frac, mut seq) = (0, 0, None);
        while let Some(open) = rest.find('{') {
            if rest[..open].contains('}') {
                return Err(invalid(TemplateProblem::StrayClose));
            }
            lit.push_str(&rest[..open]);
            let after = &rest[open + 1..];
            let Some(close) = after.find('}') else { return Err(invalid(TemplateProblem::Unclosed)) };
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
                        let width: u8 = n.parse().map_err(|_| invalid(TemplateProblem::SeqWidthNotNumber))?;
                        if !(1..=MAX_SEQ_WIDTH).contains(&width) {
                            return Err(invalid(TemplateProblem::SeqWidthRange));
                        }
                        seq = Some(width);
                        Piece::Seq(width)
                    }
                    None => return Err(invalid(TemplateProblem::UnknownToken { token: token.to_string() })),
                },
            };
            if !lit.is_empty() {
                pieces.push(Piece::Lit(std::mem::take(&mut lit)));
            }
            pieces.push(piece);
            rest = &after[close + 1..];
        }
        if rest.contains('}') {
            return Err(invalid(TemplateProblem::StrayClose));
        }
        lit.push_str(rest);
        if !lit.is_empty() {
            pieces.push(Piece::Lit(lit));
        }
        let Some(seq_width) = seq else { return Err(invalid(TemplateProblem::NeedsOneSeq)) };
        if pieces.iter().filter(|p| matches!(p, Piece::Seq(_))).count() > 1 {
            return Err(invalid(TemplateProblem::NeedsOneSeq));
        }
        if proj > 1 || frac > 1 {
            return Err(invalid(TemplateProblem::RepeatedToken));
        }
        Ok(Template { pieces, seq_width })
    }

    /// Scope-specific rule: numbers per fraction must show the fraction.
    pub fn validate_for(&self, scope: Scope) -> Result<()> {
        if scope == Scope::Fraction && !self.pieces.contains(&Piece::Frac) {
            return Err(CoreError::InvalidTemplate(TemplateProblem::FracRequired));
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
                // An empty token directly before this one had nothing to drop; the literal that
                // follows belongs to this (emitted) token and must stay.
                skip_next_lit = false;
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
        return Err(CoreError::InvalidCode(CodeProblem::TooLong { max: MAX_CODE_LEN }));
    }
    if c.chars().any(|ch| ch == '{' || ch == '}' || ch.is_whitespace() || ch.is_control()) {
        return Err(CoreError::InvalidCode(CodeProblem::InvalidChar));
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
    fn adjacent_tokens_keep_the_literal_after_the_emitted_one() {
        // An empty token followed directly by a non-empty one must not swallow the literal
        // after the non-empty one.
        assert_eq!(render("{PROJ}{FRAC}-{SEQ:2}", "", "A", 3), "A-03");
        assert_eq!(render("{FRAC}{PROJ}-{SEQ:2}", "LAM", "", 3), "LAM-03");
        assert_eq!(render("{PROJ}{FRAC}-{SEQ:2}", "LAM", "A", 3), "LAMA-03");
        assert_eq!(render("{PROJ}{FRAC}-{SEQ:2}", "", "", 3), "03");
        // Unchanged: no literal between the tokens, nothing to keep.
        assert_eq!(render("{PROJ}-{FRAC}{SEQ:2}", "", "A", 3), "A03");
        assert_eq!(render("{PROJ}-{FRAC}{SEQ:2}", "LAM", "", 3), "LAM03");
    }

    #[test]
    fn rejects_invalid_templates() {
        let long = "x".repeat(70);
        for (t, why, problem) in [
            ("", "empty", TemplateProblem::Empty),
            ("   ", "blank", TemplateProblem::Empty),
            ("LAM-{FRAC}", "no SEQ", TemplateProblem::NeedsOneSeq),
            ("{SEQ:2}-{SEQ:2}", "two SEQ", TemplateProblem::NeedsOneSeq),
            ("{SEQ}", "SEQ without width", TemplateProblem::UnknownToken { token: "SEQ".into() }),
            ("{SEQ:0}", "width 0", TemplateProblem::SeqWidthRange),
            ("{SEQ:7}", "width 7", TemplateProblem::SeqWidthRange),
            ("{SEQ:x}", "non-numeric width", TemplateProblem::SeqWidthNotNumber),
            ("{PROJ}{PROJ}{SEQ:2}", "PROJ twice", TemplateProblem::RepeatedToken),
            ("{FRAC}{PROJ}{FRAC}{SEQ:2}", "FRAC twice", TemplateProblem::RepeatedToken),
            ("{NOPE}-{SEQ:2}", "unknown token", TemplateProblem::UnknownToken { token: "NOPE".into() }),
            ("{SEQ:2", "unclosed", TemplateProblem::Unclosed),
            ("}{SEQ:2}", "stray close before a token", TemplateProblem::StrayClose),
            ("{SEQ:2}}", "stray close at the end", TemplateProblem::StrayClose),
            (long.as_str(), "too long", TemplateProblem::TooLong { max: MAX_TEMPLATE_LEN }),
        ] {
            let err = Template::parse(t).unwrap_err();
            assert!(matches!(&err, CoreError::InvalidTemplate(p) if *p == problem), "{why}: {t:?} gave {err:?}");
        }
    }

    #[test]
    fn every_template_and_code_problem_has_its_own_code() {
        let codes: Vec<&str> = [
            TemplateProblem::Empty,
            TemplateProblem::TooLong { max: 1 },
            TemplateProblem::StrayClose,
            TemplateProblem::Unclosed,
            TemplateProblem::SeqWidthNotNumber,
            TemplateProblem::SeqWidthRange,
            TemplateProblem::UnknownToken { token: "X".into() },
            TemplateProblem::NeedsOneSeq,
            TemplateProblem::RepeatedToken,
            TemplateProblem::FracRequired,
        ]
        .iter()
        .map(|p| CoreError::InvalidTemplate(p.clone()).code())
        .chain([CoreError::InvalidCode(CodeProblem::InvalidChar).code(), CoreError::InvalidCode(CodeProblem::TooLong { max: 16 }).code()])
        .collect();
        let mut unique = codes.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), codes.len(), "one code per problem: {codes:?}");
        assert!(codes.iter().all(|c| c.starts_with("template_") || c.starts_with("code_")), "{codes:?}");
        // The core's own text stays English and readable for logs; the UI never shows it.
        assert_eq!(CoreError::InvalidTemplate(TemplateProblem::UnknownToken { token: "NOPE".into() }).to_string(), "invalid ref template: unknown token {NOPE}");
        assert_eq!(CoreError::InvalidTemplate(TemplateProblem::Unclosed).to_string(), "invalid ref template: unclosed '{'");
    }

    #[test]
    fn fraction_scope_requires_frac() {
        let t = Template::parse("{PROJ}-{SEQ:2}").unwrap();
        assert!(t.validate_for(Scope::Project).is_ok());
        assert!(matches!(t.validate_for(Scope::Fraction), Err(CoreError::InvalidTemplate(TemplateProblem::FracRequired))));
        assert!(Template::parse(DEFAULT_TEMPLATE).unwrap().validate_for(Scope::Fraction).is_ok());
    }

    #[test]
    fn codes_and_markers() {
        assert_eq!(validate_code("  pc ").unwrap(), "pc");
        assert_eq!(validate_code("").unwrap(), "");
        // The rule as applied: at most 16 characters, no whitespace, braces or control characters;
        // everything else (accents, º, /, _, - ., digits) is a valid code.
        for bad in ["a b", "1 D", "a\tb", "a\u{a0}b", "{x}", "x}", "a\u{7}b"] {
            assert!(matches!(validate_code(bad), Err(CoreError::InvalidCode(CodeProblem::InvalidChar))), "{bad:?}");
        }
        assert!(matches!(validate_code(&"a".repeat(17)), Err(CoreError::InvalidCode(CodeProblem::TooLong { max: 16 }))));
        for ok in ["1D", "0.12", "PC", "1º", "R/C", "Fração", "A_1", "T-2", &"a".repeat(16)] {
            assert_eq!(validate_code(ok).unwrap(), ok, "{ok:?}");
        }
        let t = Template::parse(DEFAULT_TEMPLATE).unwrap();
        assert_eq!(marker(Scope::Project, &t, "A", 3), "3");
        assert_eq!(marker(Scope::Fraction, &t, "A", 3), "A-03");
        assert_eq!(Scope::Project.seq_key("A"), "");
        assert_eq!(Scope::Fraction.seq_key("A"), "A");
    }
}
