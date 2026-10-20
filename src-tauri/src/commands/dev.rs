//! Debug-build-only helpers for the Dev screen. This module is compiled out of release builds
//! (`#[cfg(any(debug_assertions, feature = "dev-seed"))]` in `commands/mod.rs` and on the handler
//! list in `lib.rs`); the `dev-seed` feature exists for local measurement builds only (D-026).
use std::io::Cursor;

use checkflat_core::annotation::{Annotation, Shape};
use checkflat_core::photos::{self, PhotoInput};
use checkflat_core::refs::Scope;
use checkflat_core::repo::{fractions, observations, plans, projects};
use checkflat_core::CoreError;
use tauri::AppHandle;

use crate::error::AppResult;
use crate::state::run_db;

const TEXTS: [&str; 3] = [
    "Fissura horizontal na parede da sala, junto ao rodapé, com cerca de 1,2 m de comprimento e abertura visível a olho nu. A superfície apresenta manchas de humidade na zona inferior, o que indica possível infiltração pela junta da caixilharia. Recomenda-se abrir a zona afetada, verificar a impermeabilização e refazer o acabamento após a correção.",
    "Buracos na porta do quarto; tratar e pintar. Verificar igualmente o funcionamento da fechadura, que apresenta folga, e o alinhamento da folha com o aro, responsabilidade do empreiteiro de carpintarias.",
    "Tomada da cozinha sem tampa e com cabos à vista; situação perigosa que deve ser corrigida antes da entrega. Confirmar com o instalador elétrico a conformidade do circuito e entregar o respetivo certificado.",
];

/// The source photo with a small coloured square stamped on it, so every seeded copy has
/// different bytes: Typst memoizes decoded images by content, and identical copies would hide
/// the real memory cost of 150 photos.
fn unique_copy(jpeg: &[u8], n: u32) -> Result<Vec<u8>, CoreError> {
    let mut img = image::load_from_memory(jpeg).map_err(|e| CoreError::UnreadableImage(e.to_string()))?.to_rgb8();
    let (w, h) = (img.width(), img.height());
    let colour = image::Rgb([(n * 53 % 256) as u8, (n * 97 % 256) as u8, (n * 193 % 256) as u8]);
    for y in 0..h.min(48) {
        for x in 0..w.min(48) {
            img.put_pixel(x, y, colour);
        }
    }
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&img)
        .map_err(|e| CoreError::UnreadableImage(e.to_string()))?;
    Ok(out)
}

/// One ellipse and one arrow, placed by `n` so no two photos share a drawing; every third photo
/// also gets a short freehand stroke, so the report's flattening exercises all three shapes.
fn seed_annotation(n: u32) -> Annotation {
    let k = f64::from(n % 7) / 7.0;
    let mut shapes = vec![
        Shape::Ellipse { cx: 0.3 + 0.4 * k, cy: 0.35 + 0.2 * k, rx: 0.12 + 0.05 * k, ry: 0.1 },
        Shape::Arrow { x1: 0.1 + 0.1 * k, y1: 0.85, x2: 0.3 + 0.4 * k, y2: 0.5 + 0.2 * k },
    ];
    if n % 3 == 0 {
        shapes.push(Shape::Freehand { pts: vec![[0.6, 0.2 + 0.1 * k], [0.7, 0.3], [0.8, 0.2 + 0.1 * k], [0.9, 0.3]] });
    }
    Annotation::new(shapes)
}

/// Creates `count` observations on `plan_id` through the normal save path, three photos each
/// (copies of photos already in the project, re-staged), pseudo-random positions including
/// corners, long PT descriptions, and capture times on both sides of the 2026-10-25 DST change;
/// with `annotate`, shapes on every photo (the end-of-Sprint-4 re-measure, D-026).
/// The 50-observation fixture for the report measurements (D-024).
#[tauri::command]
pub async fn dev_seed_observations(app: AppHandle, plan_id: String, count: u32, annotate: bool) -> AppResult<u32> {
    run_db(app, move |c, dir| {
        let plan = plans::get(c, &plan_id)?;
        let project = projects::get(c, &plan.project_id)?;
        let fraction = match project.seq_scope {
            Scope::Project => String::new(),
            Scope::Fraction => fractions::list(c, &project.id)?
                .first()
                .map(|f| f.code.clone())
                .ok_or_else(|| CoreError::Validation("seed: the project numbers per fraction but has none".into()))?,
        };
        let mut stmt = c.prepare("SELECT p.file_path FROM photo p JOIN observation o ON o.id = p.observation_id WHERE o.project_id = ?1 ORDER BY p.created_at")?;
        let sources: Vec<String> = stmt.query_map([&project.id], |r| r.get(0))?.collect::<Result<_, _>>()?;
        if sources.is_empty() {
            return Err(CoreError::Validation("seed: add one observation with a photo first".into()));
        }
        let bytes: Vec<Vec<u8>> = sources
            .iter()
            .map(|p| checkflat_core::paths::RelPath::new(p).and_then(|r| Ok(std::fs::read(r.resolve(dir))?)))
            .collect::<Result<_, CoreError>>()?;
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15 ^ count as u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as f64 / f64::from(1u32 << 31)
        };
        let times = ["2026-10-20T09:15:00.000Z", "2026-10-30T09:15:00.000Z", "2026-10-25T00:40:00.000Z"];
        for i in 0..count {
            let (x, y) = match i % 10 {
                0 => (0.01, 0.01),
                5 => (0.99, 0.98),
                _ => (rnd(), rnd()),
            };
            let staged: Vec<PhotoInput> = (0..3)
                .map(|j| {
                    let src = &bytes[((i * 3 + j) as usize) % bytes.len()];
                    let s = photos::stage(dir, Cursor::new(unique_copy(src, i * 3 + j)?), 0)?;
                    let annotation = annotate.then(|| seed_annotation(i * 3 + j));
                    Ok(PhotoInput { token: s.token, taken_at: times[j as usize].to_string(), annotation })
                })
                .collect::<Result<_, CoreError>>()?;
            let text = format!("{} {}", TEXTS[(i as usize) % TEXTS.len()], i + 1);
            observations::create_observation(c, dir, &plan.id, x, y, &fraction, &text, &staged)?;
        }
        Ok(count)
    })
    .await
}
