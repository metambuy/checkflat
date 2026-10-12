//! Typst compile + PDF export (D-004, D-024). The template gets its content as one JSON string
//! (`sys.inputs.data`) and its images as virtual files; fonts are embedded in the binary.
use std::time::Instant;

use typst::foundations::{Dict, IntoValue};
use typst_as_lib::TypstEngine;
use typst_layout::PagedDocument;

use crate::data::ReportInput;
use crate::{ReportError, Result};

const TEMPLATE: &str = include_str!("../assets/report.typ");
const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/LiberationSans-Regular.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../assets/fonts/LiberationSans-Bold.ttf");
const FONT_ITALIC: &[u8] = include_bytes!("../assets/fonts/LiberationSans-Italic.ttf");

#[derive(Debug)]
pub struct Rendered {
    pub pdf: Vec<u8>,
    pub pages: usize,
    pub compile_ms: u64,
    pub pdf_ms: u64,
}

pub fn render(input: &ReportInput) -> Result<Rendered> {
    let engine = TypstEngine::builder()
        .fonts([FONT_REGULAR, FONT_BOLD, FONT_ITALIC])
        .with_static_source_file_resolver([("main.typ", TEMPLATE)])
        .with_static_file_resolver(input.files.iter().map(|(name, bytes)| (name.as_str(), bytes.as_slice())))
        .build();
    let mut inputs = Dict::new();
    inputs.insert("data".into(), serde_json::to_string(&input.data)?.into_value());

    let started = Instant::now();
    let compiled = engine.compile_with_input("main.typ", inputs);
    for w in &compiled.warnings {
        log::warn!("typst: {}", w.message);
    }
    let result = (|| {
        let doc: PagedDocument = compiled.output.map_err(|e| ReportError::Typst(e.to_string()))?;
        let compile_ms = started.elapsed().as_millis() as u64;
        let started = Instant::now();
        let pdf = typst_pdf::pdf(&doc, &Default::default()).map_err(|e| ReportError::Typst(format!("pdf export: {e:?}")))?;
        Ok(Rendered { pdf, pages: doc.pages().len(), compile_ms, pdf_ms: started.elapsed().as_millis() as u64 })
    })();
    // Typst memoizes decoded images and layout results across compiles; drop them now.
    comemo::evict(0);
    result
}
