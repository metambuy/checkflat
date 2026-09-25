//! Sprint 0 — Spike C. One page: title, image, text block.
//! Two engines behind Cargo features so each can be linked into the app alone
//! and its cost (APK size, build time, render time) measured in isolation.

#[cfg(feature = "printpdf")]
mod printpdf_engine;
#[cfg(feature = "typst")]
mod typst_engine;

pub const FONT_REGULAR: &[u8] = include_bytes!("../assets/LiberationSans-Regular.ttf");
pub const FONT_BOLD: &[u8] = include_bytes!("../assets/LiberationSans-Bold.ttf");
pub const SAMPLE_JPG: &[u8] = include_bytes!("../assets/sample.jpg");

/// Content of the one-page sample report. Identical for both engines.
pub struct Report<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
    /// JPEG bytes.
    pub image: &'a [u8],
    pub paragraph: &'a str,
}

impl Default for Report<'static> {
    fn default() -> Self {
        Report {
            title: "Relatório de visita — Obra X",
            subtitle: "Observação nº 12 · Piso 3 · 24/09/2026",
            image: SAMPLE_JPG,
            paragraph: "Fissura horizontal na parede da sala, junto ao rodapé, com cerca de 1,2 m de \
comprimento e abertura visível a olho nu. A superfície apresenta manchas de humidade na zona \
inferior, o que indica possível infiltração pela junta da caixilharia. Recomenda-se abrir a \
zona afetada, verificar a impermeabilização e refazer o acabamento após a correção. \
Reavaliar na próxima visita; o item mantém-se em aberto até confirmação do empreiteiro. \
Ação: empreiteiro geral. Prazo indicativo: duas semanas.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Typst,
    Printpdf,
}

impl Engine {
    pub fn parse(s: &str) -> Option<Engine> {
        match s {
            "typst" => Some(Engine::Typst),
            "printpdf" => Some(Engine::Printpdf),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Engine::Typst => "typst",
            Engine::Printpdf => "printpdf",
        }
    }

    /// Engines compiled into this binary.
    pub fn available() -> Vec<Engine> {
        let mut v = Vec::new();
        if cfg!(feature = "typst") {
            v.push(Engine::Typst);
        }
        if cfg!(feature = "printpdf") {
            v.push(Engine::Printpdf);
        }
        v
    }
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// Render the report to PDF bytes with the chosen engine.
#[allow(unused_variables)] // `report` is unused when no engine feature is enabled
pub fn render(engine: Engine, report: &Report) -> Result<Vec<u8>, Error> {
    match engine {
        #[cfg(feature = "typst")]
        Engine::Typst => typst_engine::render(report),
        #[cfg(feature = "printpdf")]
        Engine::Printpdf => printpdf_engine::render(report),
        #[allow(unreachable_patterns)]
        other => Err(format!("engine {} not compiled in", other.name()).into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(engine: Engine) {
        let started = std::time::Instant::now();
        let pdf = render(engine, &Report::default()).expect("render failed");
        eprintln!("[measure] {} render {} ms, {} bytes", engine.name(), started.elapsed().as_millis(), pdf.len());
        assert!(!pdf.is_empty(), "{} produced empty output", engine.name());
        assert!(pdf.starts_with(b"%PDF"), "{} output is not a PDF", engine.name());
        // Write next to target/ so the two outputs can be compared visually.
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../out");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join(format!("report-{}.pdf", engine.name())), &pdf).unwrap();
    }

    #[cfg(feature = "typst")]
    #[test]
    fn typst_outputs_pdf() {
        check(Engine::Typst);
    }

    #[cfg(feature = "printpdf")]
    #[test]
    fn printpdf_outputs_pdf() {
        check(Engine::Printpdf);
    }
}
