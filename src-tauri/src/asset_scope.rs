//! The asset-protocol scope in `tauri.conf.json` against the files the WebView actually loads
//! (D-015, D-022): plan tiles and stored photos under `projects/`, staged photo JPEGs under `tmp/`
//! and nothing else in `tmp/` (staged PDFs, half-written `.part` files). Patterns are matched like
//! Tauri does for the asset protocol: `glob` with a literal separator and a literal leading dot.
use checkflat_core::paths::{photo_file, plan_tiles_dir, staging_file, staging_photo};
use glob::{MatchOptions, Pattern};
use std::path::Path;

fn scope() -> Vec<Pattern> {
    let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json parses");
    conf["app"]["security"]["assetProtocol"]["scope"]
        .as_array()
        .expect("asset scope is a list")
        .iter()
        .map(|p| {
            let p = p.as_str().expect("pattern").replace("$APPDATA", "/data/app");
            Pattern::new(&p).expect("valid glob")
        })
        .collect()
}

fn allowed(scope: &[Pattern], rel: &str) -> bool {
    let opts = MatchOptions { case_sensitive: true, require_literal_separator: true, require_literal_leading_dot: true };
    let path = Path::new("/data/app").join(rel);
    scope.iter().any(|p| p.matches_path_with(&path, opts))
}

#[test]
fn asset_scope_serves_what_the_viewer_loads_and_only_that() {
    let scope = scope();
    let (pid, id, token) = ("0190aaaa-bbbb-7ccc-8ddd-eeeeeeeeeeee", "0190ffff-bbbb-7ccc-8ddd-eeeeeeeeeeee", "01900000-bbbb-7ccc-8ddd-eeeeeeeeeeee");

    // Served: plan tiles and the plan PDF (the tile generator fetches it), stored photos, staged photos.
    let tile = format!("{}/1024/0_0.webp", plan_tiles_dir(pid, id));
    assert!(allowed(&scope, &tile), "{tile}");
    assert!(allowed(&scope, &format!("projects/{pid}/plans/{id}.pdf")));
    assert!(allowed(&scope, photo_file(pid, id).as_str()));
    assert!(allowed(&scope, staging_photo(token).as_str()), "staged thumbnails must load");

    // Not served: staged plan PDFs, half-written staging files, the database, anything else in tmp/.
    assert!(!allowed(&scope, staging_file(token).as_str()), "a staged PDF is not a displayable asset");
    assert!(!allowed(&scope, &format!("tmp/{token}.jpg.part")));
    assert!(!allowed(&scope, &format!("tmp/{token}.png")));
    assert!(!allowed(&scope, "tmp/sub/x.jpg"), "no directories under tmp/");
    assert!(!allowed(&scope, "checkflat.db"));
    assert!(!allowed(&scope, "tmp/../checkflat.db"));
}
