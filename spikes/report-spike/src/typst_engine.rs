use crate::{Error, Report, FONT_BOLD, FONT_REGULAR};
use typst::foundations::{Dict, IntoValue};
use typst_as_lib::TypstEngine;

const TEMPLATE: &str = include_str!("../assets/report.typ");

pub fn render(report: &Report) -> Result<Vec<u8>, Error> {
    let engine = TypstEngine::builder()
        .fonts([FONT_REGULAR, FONT_BOLD])
        .with_static_source_file_resolver([("main.typ", TEMPLATE)])
        .with_static_file_resolver([("photo.jpg", report.image)])
        .build();

    let mut inputs = Dict::new();
    inputs.insert("title".into(), report.title.into_value());
    inputs.insert("subtitle".into(), report.subtitle.into_value());
    inputs.insert("paragraph".into(), report.paragraph.into_value());

    let compiled = engine.compile_with_input("main.typ", inputs);
    let doc = compiled.output.map_err(|e| format!("typst compile: {e:?}"))?;
    let pdf = typst_pdf::pdf(&doc, &Default::default())
        .map_err(|e| format!("typst pdf export: {e:?}"))?;
    Ok(pdf)
}
