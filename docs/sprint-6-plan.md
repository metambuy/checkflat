<!-- Part 6a approved 2026-10-22 (plan mode, Claude Code), with Martín's decisions: copy to cache/reports/ on Open/Share, remove the template's FileProvider, a CI guard on the APK's FileProvider, PT labels "Abrir"/"Partilhar". Its implementation record is docs/decisions.md D-028.
Part 6b approved 2026-10-26 (plan mode, Claude Code), with Martín's decisions: all crops grey at 16 levels; no Windows machine, so the friend runs the Windows test during the preview after the merge and Windows gets more CI coverage (core tests, file-locking tests); a build number on Windows too; two Windows checklists (technical in D-029, a short PT one for the tester); the annotation hint matches the input; the v0.1 super review is set up by Martín, not started here. Its implementation record is docs/decisions.md D-029. -->

# Sprint 6 — report Open/Share (6a), Windows save and smaller report images (6b) (plan)

Part **6a** is planned in full below. Part **6b** has its own plan section after it ([Part 6b](#part-6b--windows-open--save-a-copy-smaller-report-images-app-switch-during-generation-v010)): Windows Open / Save a copy, the smaller report images (16-level greyscale crops and 640 px / q75 photos, D-024/D-026/D-027), the HyperOS app-switch check and v0.1.0. It is on branch `sprint-6b` from `main` (`fc362f1`). The end-of-Sprint-6 super review follows 6b and is set up by Martín.

# Part 6a — Android Open/Share for the report, app version

## Context
Sprint 6 (plan §4, M9) gives the report its real action: on Android the user opens the PDF in a viewer or shares it (Gmail, WhatsApp, Drive). This ships before the friend preview, so the report action has to be clear and the tester has to be able to say which build he has. Split agreed with Martín: **6a** = Android Open/Share, the report action, the About line, docs. **6b** = Windows sharing/save, smaller report images (D-024/D-026/D-027), the HyperOS app-switch check, the super review. Branch `sprint-6a` from `main` (`a2feef6`); one PR; stop when it is open with CI green.

Verified facts that shape the design:
- **`reports/` can't be served by a FileProvider where it lives today.** Tauri's `app_data_dir()` on Android is `activity.dataDir`, the package root (tauri 2.11.6 as pinned in `Cargo.lock`, `src/path/android.rs:138`, `PathPlugin.kt getDataDir`). FileProvider documents only `files-path`, `cache-path` and the `external-*` paths, with no element for the data-dir root. `root-path` exists only in the source (`TAG_ROOT_PATH` → `/`). → **Decided (Martín, 2026-10-22): copy on Open/Share** into `cacheDir/reports/<project id>/<readable name>.pdf` and expose only `<cache-path name="reports" path="reports/"/>`. `app_cache_dir()` on Android = `activity.cacheDir` (`android.rs:152`).
- The plugin's provider (`CaptureFileProvider`, authority `<applicationId>.cameracapture`, `cache-path captures/`) is the one to extend. **The app template still declares `androidx.core.content.FileProvider` `<applicationId>.fileprovider` with `external-path "."` + `cache-path "."`** (`src-tauri/gen/android/app/src/main/AndroidManifest.xml`, `res/xml/file_paths.xml`). It has been unused since D-022(b), and `tauri-plugin-dialog`/`-fs` don't reference a FileProvider (checked in the registry sources). → **Decided: remove it in 6a.**
- Reports today: `paths::write_report` writes `reports/<pid>-<stamp>.pdf`, prunes the project's older PDFs **before** writing, and keeps one PDF per project. Nothing records the page count, and project delete leaves the PDF behind (`repo/projects.rs:150`).
- Report UI today: a temporary block in `ProjectScreen.svelte:24-40,109-126` (language select, Generate, path + timings).
- Version: `versionName` = `tauri.conf.json` `version` (`0.0.1`) via `tauri.properties`; `versionCode` = CI `github.run_number` (`ci.yml` `-c '{"bundle":{"android":{"versionCode":…}}}'`). Only Android's PackageManager knows the installed pair.
- The app already calls the plugin from Rust (`commands/plans.rs:46` `handle.camera_capture().display_name(uri)` in `spawn_blocking`). Plugin commands run on the Android main thread: Rust → `run_on_android_context` → wry dispatch. So `startActivity` inside a command is fine, and the Rust side must stay in `spawn_blocking`. Kotlin's `invoke.reject(msg, code)` puts `code` at the top level (`Invoke.kt:48-58`), where Rust reads it as `ErrorResponse.code`. Release minification keeps `@Command`/`@InvokeArg` (tauri-api `proguard-rules.pro`).
- **Rust `log::` writes nowhere on Android** (no logger in `Cargo.lock`). Report timings reach the device only through the frontend's `devlog()`, which runs in spike builds only (`devlog.ts:7-9`).
- **CI runs only on `pull_request` and on pushes to `main`** (`ci.yml:4-7`), so the branch needs a draft PR after its first code push, as 4b had (runs 56–61). Every push to the PR, docs too, starts a run with a new versionCode.
- **Community plugins: I found no strong reason to use one** (checked 2026-10-22, recorded in D-028).
  - The official `tauri-plugin-opener` opens only URLs on Android: no FileProvider, and `open_path` is ignored on mobile. Its issue #2383 is still open.
  - There is no official share plugin.
  - `tauri-plugin-sharesheet` shares text only and is abandoned.
  - `tauri-plugin-share` reuses the template's broad provider and adds storage permissions.
  - `tauri-plugin-sharekit` declares the same `.fileprovider` authority as the template, so the broad paths apply.
  - `tauri-plugin-vnidrop-share` has a narrow provider but copies in any absolute path; it is a release candidate.
  - None of them offers `ACTION_VIEW` for a local file, so we'd need our own Kotlin for Open anyway.
- androidx.core 1.9.0 is already in the plugin: `ShareCompat.IntentBuilder(Context)` since 1.5.0, `PackageInfoCompat` since 1.1.0. No new dependency.

### Android docs this relies on (cited in D-028 and in the Kotlin comments)
1. FileProvider reference: path elements, `query()` returns `DISPLAY_NAME` = the file's name (so the copy is named for the recipient), "Granting Temporary Permissions to a URI". https://developer.android.com/reference/androidx/core/content/FileProvider · setup: https://developer.android.com/training/secure-file-sharing/setup-sharing
2. Grant via intent flags ("temporary and expire automatically when the receiving app's task stack is finished"): https://developer.android.com/training/secure-file-sharing/share-file
3. Package visibility, Android 11+: "invoke the intent and handle the ActivityNotFoundException exception if no app is available"; the "Open a file" case; "Grant URI access" with `ACTION_VIEW` + `FLAG_GRANT_READ_URI_PERMISSION`: https://developer.android.com/training/package-visibility/use-cases
4. Sharesheet, "Send binary content" (`EXTRA_STREAM`, MIME type, per-URI permissions through FileProvider, `createChooser`): https://developer.android.com/training/sharing/send
5. `Intent.createChooser` copies the read/write grant flags and ClipData to the chooser: https://developer.android.com/reference/android/content/Intent#createChooser(android.content.Intent,%20java.lang.CharSequence) · `ShareCompat.IntentBuilder`: https://developer.android.com/reference/androidx/core/app/ShareCompat.IntentBuilder. Since androidx.core 1.5.0-beta01, `getIntent()` sets ClipData plus `FLAG_GRANT_READ_URI_PERMISSION` for streams (release note "Set clip data and grant uri read permission when sharing streams using ShareCompat"): https://developer.android.com/jetpack/androidx/releases/core
6. `PackageInfo.getLongVersionCode()` (API 28; `versionCode` deprecated) and `PackageInfoCompat.getLongVersionCode`: https://developer.android.com/reference/android/content/pm/PackageInfo · https://developer.android.com/reference/androidx/core/content/pm/PackageInfoCompat

---

## Design

### Storage: latest report + metadata (core)
- New core module **`crates/checkflat-core/src/reports.rs`**. `report_file`, `write_report` and `prune_reports` move here from `paths.rs`, with their test; `paths.rs` keeps `REPORTS_DIR`. The report-file logic triples, so it gets one home.
- `reports/<pid>-<stamp>.pdf` stays as it is (same layout on Windows). A sidecar **`reports/<pid>.json`** holds `ReportMeta { v: 1, file, name, lang, pages, bytes, createdAt }`. `name` is the readable name, computed **at generation**, so a later project rename doesn't change it under the PDF's own cover.
- `write_report(data_dir, pid, pdf, meta)`: PDF `.part` → fsync → rename, then the JSON the same way, **then** prune the project's other PDFs and `.part` files. Today the prune runs first; moving it after means a failed generation keeps the previous report usable.
- `latest(data_dir, pid) -> Option<ReportMeta>`: returns `None` when the JSON is missing or unparseable, or when its file is missing or not a regular file, or its size ≠ `bytes`. Reads never fail. Reports from before 6a have no JSON, so they show as "no report yet" and are pruned at the next generation.
- **Every function that takes a project id checks it with `ids::is_id` first** (`Validation` otherwise). The id comes from JS and goes into data and cache paths, and `report_file` would panic on `..` (`paths.rs:166`).
- `display_name(project_name, date_iso) -> String` → `"<name> - yyyy-mm-dd.pdf"`:
  - control characters and `/ \ : * ? " < > |` become spaces
  - whitespace collapses
  - the name part is capped at **100 bytes** on a char boundary (filenames are limited to 255 bytes; Portuguese accents are multibyte)
  - spaces, dots and hyphens are trimmed at both ends **after** the cut, so a cut can't leave a trailing space
  - accents are kept
  - an empty result falls back to `Checkflat`
- `share_copy(data_dir, pid, cache_reports_dir) -> PathBuf`: copies the latest report to `<cache_reports_dir>/<pid>/<name>` (`.part` + rename; an open reader of an older copy keeps its inode). The per-project subfolder means two projects with the same name on the same day can't overwrite each other's pending share. Before copying, it purges copies older than 24 h and stray `.part` files, the same rule as the plugin's `purgeOldCache`.
- `remove_for_project(data_dir, pid)`: the project's PDFs, `.part` files and JSON. **Called from `repo/projects.rs::delete`**, which today leaves the report (client data) behind. `commands/projects.rs::delete_project` also removes `app_cache_dir()/reports/<pid>/`.
- **Sweep:** `sweep_orphans` gains a step that removes `reports/<uuid>-*.pdf|.part` and `reports/<uuid>.json` of projects that no longer exist. It cleans up deletes from before 6a and a kill between the row delete and the file removal. Reports are derived and can be regenerated, and their project was deleted on purpose, so this step removes them instead of quarantining. It keeps the sweep's existing guards: fresh-DB skip, UUID names only, real files only, contained. Tested in `tests/sweep.rs`.
- **One instant per report.** `ReportSource` gains `created_at` (UTC RFC 3339, set in `collect`). `prepare` prints the cover date from it (`clock.date(&created_at)` instead of `clock.today()`, `data.rs:266`). Before `prepare` moves the source, the command reads `created_at`, `project.name` and `clock.iso_date(&created_at)`. So the cover date, the name's date and `createdAt` agree, also near midnight. `prepare`'s signature and its 8 test call sites stay as they are. The `<stamp>` in the stored file name is only an id (`now_compact()`).
- `checkflat-report/src/time.rs`: `Clock::iso_date(iso) -> "yyyy-mm-dd"` (local date of an instant).

### Generation command (Tauri)
- `generate_report` returns `{ report: ReportMeta, stats, compileMs, pdfMs, totalMs }`. Normal builds no longer show the path or timings. Spike builds (`VITE_SPIKES=1`, the D-026 measurement builds) keep the timing line under the card and write it to `dev.log`, which 6b's image-size work needs.
- A process-wide **single-flight guard** (`Mutex<Option<project id>>` + drop guard) rejects a second run with `report_busy`: two runs at once would double the ≈ 635 MB peak (D-026). **`delete_project` refuses (`report_busy`) the project being generated.** Otherwise a delete during the run would be followed by the report write recreating its files.
- The `dev-seed` copy to Documents stays.
- New: `latest_report(project_id) -> Option<ReportMeta>`, `open_report(project_id)` and `share_report(project_id, title)`. The last two make the share copy in `app_cache_dir()/reports` (`run_fs`), then call the plugin in `spawn_blocking`. **JS never passes a path:** the backend resolves the file from the project id.
- `app_version() -> { version, build: Option<String> }`: on Android from the plugin (PackageManager), on desktop `package_info().version` with no build.
- Errors: a plugin rejection with code `no_app` → `no_pdf_app` (Open) / `no_share_app` (Share). `no_report` when the latest report is gone. `report_busy`.

### Android plugin (extend `camera-capture`, the app's Android bridge since D-013)
- `camera_capture_paths.xml`: add `<cache-path name="reports" path="reports/" />`, so the paths are `captures/` + `reports/` in the cache.
- `CameraCapturePlugin.kt`, three new `@Command`s:
  - **`viewPdf {path}`**: checks that `path` canonicalises inside `cacheDir/reports/` and is a file (else reject `invalid_path`). Then `FileProvider.getUriForFile(…, "<pkg>.cameracapture", file)` and `Intent(ACTION_VIEW).setDataAndType(uri, "application/pdf").addFlags(FLAG_GRANT_READ_URI_PERMISSION)`. `startActivity` runs on the UI thread; **`ActivityNotFoundException` → reject code `no_app`**. No `resolveActivity` and no `<queries>`.
  - **`sharePdf {path, title}`**: same check, then `ShareCompat.IntentBuilder(activity).setType("application/pdf").setStream(uri).setChooserTitle(title).createChooserIntent()`. ShareCompat sets ClipData and the read grant; `createChooser` carries them through the chooser. `startActivity`, with `ActivityNotFoundException` → `no_app`.
  - **`appVersion`**: `getPackageInfo(packageName, 0)` → `{versionName, versionCode: PackageInfoCompat.getLongVersionCode(info)}`.
  - Log lines `view:`/`share:` (URI path, bytes, result) under the existing tag `CheckflatPick`, so one `adb logcat -s CheckflatPick` filter covers the bridge.
- Plugin Rust: `models.rs` (`PdfArgs`, `AppVersion`), `mobile.rs` (`view_pdf`, `share_pdf`, `app_version`), `desktop.rs` (`Unsupported`; `app_version` from `package_info`), `error.rs` (`Error::rejected_code() -> Option<&str>`). They are callable from Rust only and **not added to the plugin's JS `invoke_handler` or permissions**. README updated.
- Template provider removed from `gen/android/.../AndroidManifest.xml` and `res/xml/file_paths.xml` deleted. Nothing outside `docs/` references them. `CaptureFileProvider.kt`'s comment, which explains the subclass by the template provider, is updated. Afterwards the merged manifest is checked: exactly **one FileProvider** (`CaptureFileProvider`) and no `com.checkflat.app.fileprovider`. `androidx.startup.InitializationProvider` stays; it is not a FileProvider.

### CI guard: one narrow FileProvider (Martín, plan review)
`gen/android` can be regenerated by the Tauri CLI, which would silently bring back the broad template provider. So the `android-release` job checks the **release APK itself**, like the dev-seed step.
- New script **`scripts/check-fileprovider.sh <apk> <build-tools dir>`**, `set -euo pipefail`, **fail closed**: it fails if `aapt2` is missing or not executable, if any dump fails or is empty, or if any expected element can't be found.
  1. `aapt2 dump xmltree --file AndroidManifest.xml <apk>`. Exactly **one** `<provider>` carries the `android.support.FILE_PROVIDER_PATHS` meta-data. Its `android:name` is `com.checkflat.cameracapture.CaptureFileProvider`, its `android:authorities` is `com.checkflat.app.cameracapture` and `android:exported` is false. The string `com.checkflat.app.fileprovider` appears nowhere.
  2. The meta-data's `android:resource` id (`@0x7f…`) → `aapt2 dump resources <apk>` → the file path in the APK. The path is resolved, not assumed, because release builds may rename resource paths.
  3. `aapt2 dump xmltree --file <that path> <apk>`: the `<paths>` element has exactly two children, both `cache-path`, with `path` `captures/` and `reports/`. Nothing else.
- `ci.yml`: a step "Only the narrow FileProvider in the release APK" after the dev-seed step, running the script with `$ANDROID_SDK_ROOT/build-tools/36.0.0`. It is written to `$GITHUB_STEP_SUMMARY` as one line.
- **Tested locally both ways before it goes into CI** (recorded in D-028):
  - the current `main` APK (the CI run 62 artifact via `gh run download`, or run 61's, the same code; template provider present) → **fails**, naming the extra provider and its broad paths
  - the 6a APK → **passes**
  - a mutated case, e.g. a missing `aapt2` path → fails

### Report action (UI)
- New **`src/lib/report/ReportCard.svelte`** replaces ProjectScreen's temporary block (`ProjectScreen.svelte:24-40,109-126`):
  - **Report language** select (EN/PT, defaulting to the UI language) and **Generate report**.
  - While generating: the button and the select are disabled, with an indeterminate native `<progress>` and "Generating report…" (`aria-live`).
  - Then: "Latest report: 22.10.2026 14:05 · PT · 69 pages · 17.6 MB" (I added the language because only one report is kept per project and it can be EN or PT), plus **Open** and **Share** (Android only; Windows comes in 6b). Both are disabled while generating and during the brief copy.
  - "N photo(s) could not be read" stays.
  - On mount, `latestReport` fills the line, so the latest report is there after leaving and returning, and after a restart.
- **`src/lib/report/jobs.ts`** (pure, tested) holds a per-project in-flight promise at module level. If the user leaves during the ≈ 20 s run and comes back, the card shows "Generating…" again and picks up the result, so the remounted button can't start a second run.
- **`src/lib/report/format.ts`** (pure, tested): `dd.mm.yyyy hh:mm` local time, size in MB/KB with the language's decimal separator (`17.6` / `17,6`), and the "1 page / N pages" label.
- `ErrorBanner`: `no_pdf_app` → "No app to open PDFs", `no_share_app`, `no_report`, `report_busy`.
- **About line** at the bottom of the Projects screen, small and muted: "Version 0.0.1 (build 63)", plus "· debug" for debug builds (`platform_info.debug`, which `platform.ts` will cache next to `os`); without a build number, "Version 0.0.1". The version stays 0.0.1; the bump to 0.1.0 belongs to the v0.1 release after 6b.
- i18n EN/PT (key parity is guarded by `en_and_pt_have_identical_keys`):
  - `report.section`, `report.latest`, `report.pages`/`report.pages_one`, `report.none`, `report.share_title`
  - **`report.open` "Open" / "Abrir", `report.share` "Share" / "Partilhar"**, matching the tester's guide; `report.generate` stays "Gerar relatório"
  - `report.generating` → "Generating report…" / "A gerar o relatório…"
  - `error.no_pdf_app` "No app to open PDFs" / "Nenhuma aplicação para abrir PDFs", `error.no_share_app`, `error.no_report`, `error.report_busy`
  - `about.version`, `about.version_only`
  - `report.done` removed.
  - None of these collide with the Typst template's `report.*` labels.

---

## Steps (commit per step)
1. `git checkout -b sprint-6a main`. **First commit: this plan as `docs/sprint-6-plan.md`** (6a in full, 6b listed as the next part).
2. **Core** `reports.rs` (moved and new functions above), with `projects::delete` calling `remove_for_project`. Tests:
   - display_name cases: `Rua 5/7: "Bloco A"`, leading/trailing dots and spaces, control characters, `///` → `Checkflat`, a 100-byte cut on `ç`, accents kept
   - write → meta → prune-after-success; a failed write keeps the previous report and its meta
   - `latest()` returns `None` for missing, garbled and size-mismatched meta
   - share_copy: name, overwrite in place, 24 h purge (mtime via `File::set_modified`), per-project subfolder
   - a non-UUID id is refused
   - project delete removes the report files (`tests/projects.rs`)
   - the sweep removes orphaned reports and keeps live ones, and skips on a fresh DB (`tests/sweep.rs`)
3. **Report crate**: `ReportSource.created_at`, cover date from it, `Clock::iso_date` (tested across the Lisbon DST change and midnight).
4. **Plugin**: Kotlin commands, `reports/` path, Rust models/mobile/desktop/error, README; template provider removed, `CaptureFileProvider.kt` comment.
4b. **CI guard**: `scripts/check-fileprovider.sh`, tested locally both ways (run 62 `main` APK → fail; a local 6a release APK → pass; missing `aapt2` → fail), then the `ci.yml` step.
5. **Commands**: `generate_report` (meta, guard with the project id), `latest_report`, `open_report`, `share_report`, `app_version`; `delete_project` (busy check, cache copies); handlers registered in `lib.rs`; error mapping; guard unit test. **Push and open a draft PR** so CI runs from here on.
6. **Frontend**: `api.ts` types and calls, `report/format.ts` + `jobs.ts` + tests, `ReportCard.svelte`, `ProjectScreen.svelte`, the About line in `ProjectsScreen.svelte`, `platform.ts` (`debug`), `ErrorBanner.svelte`, i18n.
7. **Docs**:
   - **D-028** (context, options: copy vs `files/` storage vs `root-path`, own plugin vs community plugins, ShareCompat vs a bare `ACTION_SEND`; the CI FileProvider guard and its local fail/pass runs; outcome 6a; citations; the P30 table with Martín's words; open items for 6b)
   - **CHANGELOG**: the 4b entry exists (`CHANGELOG.md:3`) but lacks the post-touch-check review fix (1e, `9d8b9ec`) and says 118 frontend tests (122 after 1e), so I'll add a bullet and correct the count; add "Sprint 6a"
   - **D-027 `decisions.md:560`**: "CI run 58 (the PR's final head)" is outdated. The final head was `9d8b9ec` = **CI run 61** (run 62 = the merge on `main`): green, APK **38,547,398 B** (versionCode 61; +192 B over run 58, the review-1e fix). Run 58 stays as the touch-checked build.
   - `checkflatPLAN.md`: §3 rows (Android bridge: open/share; Report: one latest per project + metadata), §4 Sprint 6 split into 6a/6b.
8. CI green (checks, android-release incl. the dev-seed guard, **the FileProvider guard** and the APK size line, windows) → emulator checks → P30 check by Martín on the CI build → D-028 outcome → PR ready for review → **stop** (6b only when told). **D-028 records the touch-checked run and the final-head run separately.** The docs commits after the touch check start new runs with new versionCodes; that mix-up is the one fixed in D-027.

## Files touched
- Core: `crates/checkflat-core/src/{reports.rs (new), paths.rs (moved functions, `relpath_rules` test, sweep step), lib.rs, repo/projects.rs}`, `tests/{projects.rs, sweep.rs}`
- Report: `crates/checkflat-report/src/{time.rs, data.rs}`
- Tauri: `src-tauri/src/commands/{report.rs, projects.rs}`, `src-tauri/src/lib.rs` (handlers, `app_version`), `src-tauri/src/error.rs` (plugin error codes)
- Plugin: `src-tauri/plugins/camera-capture/android/src/main/java/{CameraCapturePlugin.kt, CaptureFileProvider.kt}`, `.../res/xml/camera_capture_paths.xml`, `src/{models.rs, mobile.rs, desktop.rs, error.rs}`, `README.md`
- Android template: `src-tauri/gen/android/app/src/main/AndroidManifest.xml`, `.../res/xml/file_paths.xml` (deleted)
- CI: `scripts/check-fileprovider.sh` (new), `.github/workflows/ci.yml` (one step)
- Frontend: `src/lib/report/{ReportCard.svelte, format.ts, format.test.ts, jobs.ts, jobs.test.ts}` (new), `src/lib/screens/{ProjectScreen.svelte, ProjectsScreen.svelte}`, `src/lib/components/ErrorBanner.svelte`, `src/lib/{api.ts, platform.ts}`, `i18n/{en,pt}.json`
- Docs: `docs/sprint-6-plan.md` (new), `docs/decisions.md` (D-028, D-027 fix), `docs/checkflatPLAN.md`, `CHANGELOG.md`

## Verification
- Unit: `pnpm check && pnpm test`; `cargo test --workspace --all-features`; `cargo clippy --workspace --all-features --all-targets -- -D warnings`.
- **Emulator** (Phone_API35, debug build, `adb install -r`, data kept):
  - Generate EN and PT. The progress state shows; leave the screen mid-run and come back (still generating, then the line); restart (the line stays).
  - Open with no PDF viewer → "No app to open PDFs". Check first with `pm query-activities --brief -a android.intent.action.VIEW -d content://com.checkflat.app.cameracapture/reports/x.pdf -t application/pdf`: with `-d`, viewers whose filters declare the `content` scheme match too. If a viewer exists, `pm disable-user` it for the check and re-enable it.
  - Share → Sharesheet opens.
  - The project `Rua 5/7: "Bloco A"` → copy name under `cache/reports/<pid>/` (`run-as … ls`).
  - `adb shell content query --uri content://com.checkflat.app.cameracapture/reports/…` → SecurityException (not exported).
  - The merged manifest has exactly one FileProvider, `CaptureFileProvider`, and no `.fileprovider` (`aapt2 dump xmltree --file AndroidManifest.xml <apk>`).
  - The About line.
  - The card at 360 CSS px (`wm density 480`) in EN and PT.
  - Delete a test project → its `reports/<pid>*` and `cache/reports/<pid>/` are gone. Deleting the project that is generating is refused.

## P30 checklist (Martín, by touch, CI build of the PR; `adb install -r` over the current build, data kept)
1. **About line** on the Projects screen reads "Version 0.0.1 (build N)" with N = the CI run number of the installed APK.
2. **"Perf A1 4a"** (50 × 3, annotated, ≈ 18 MB). The card says "no report yet": reports from before 6a have no metadata. Choose **PT** → **Generate**: the button and language are disabled, with the moving bar and "A gerar o relatório…". Go Back to Projects and return mid-run: still generating. At the end: date/time, PT, pages, ≈ 18 MB, Open + Share.
3. **Open** → the PDF opens in a viewer (note which app); pages and photos look right; Back returns to Checkflat on the project screen.
4. **Share → Gmail**: the attachment is named "Perf A1 4a - yyyy-mm-dd.pdf", ≈ 18 MB (under Gmail's 25 MB). Send it to yourself; it arrives and opens.
5. **Share → WhatsApp**: sent as a document with the same name to a chat with yourself; it opens.
6. **Share → Drive**: "Save to Drive" shows the same name; the upload completes and the file opens in Drive.
7. **EN** → Generate again: the line updates (EN, new time); Share once more to any app → the new file.
8. Swipe the app away and restart: the line is still there and Open works.
9. **No-viewer message (only if you want to test it)**: Settings → Apps → disable the PDF viewer(s) Open used → Open shows "Nenhuma aplicação para abrir PDFs" / "No app to open PDFs" → re-enable them. (The emulator covers this regardless.)
10. Anything odd: a slow Open/Share (the copy should take well under a second), a chooser missing apps, a wrong name.

Pass: 1–8 as described (9 if tested). I record the results in D-028 with your words, as in D-027.

---

# Part 6b — Windows Open / Save a copy, smaller report images, app switch during generation, v0.1.0

## Context
6b is the last part before v0.1, the friend field test. 6a (D-028, merged as `fc362f1`) gave Android Open/Share and the report card. 6b covers four things:
- **Windows report actions:** M9 "save (Windows)".
- **Smaller report images (required):** the P30 peak is 641.6 MB against the ≈ 650 MB limit, and at 1.5 s sampling the true peak is likely ≥ 650 MB. The annotated 50 × 3 report is 18.4 MB against ≈ 20 MB email limits (D-024, D-026, D-027).
- **App switch during generation:** HyperOS kills background apps.
- **The 0.1.0 version.**

Branch `sprint-6b` from `main` (`fc362f1`), one PR, decision **D-029**. Martín's decisions (plan review, 2026-10-26):
- **All crops grey at 16 levels.** Colour crops become grey too; the pin and the red box are vectors and stay red.
- **No Windows machine here.** The friend runs the Windows test on his laptop during the preview, after this PR is merged. So Windows gets more CI coverage: the core and report tests run on the windows job, plus file-locking tests. D-029 marks the Windows results "pending preview"; they arrive later in a docs-only commit.
- **A build number on Windows too:** "Versão 0.1.0 (build N)" on both platforms.
- **Two Windows checklists:** the technical one in D-029, and a short PT one for the tester (plain text, for his guide).
- **The annotation hint matches the input:** desktop gets a mouse wording, Android is unchanged.
- **The v0.1 super review is not started here.** Martín sets it up; 6b stops when the PR is open with CI green and the checks are recorded.

Facts from the code that shape the design:
- **Crops** (`crates/checkflat-report/src/snapshot.rs`): `crop()` → `encode()` writes L8 when every pixel's chroma ≤ `GREY_CHROMA_MAX` (8), RGB otherwise. **`overview()` uses the same `encode()`**, so quantising there would also hit the overview.
  - typst-pdf re-deflates raw samples at 8 bits per component (`typst-pdf-0.15.1/src/image.rs`).
  - So a 4-bit PNG gains nothing in the PDF: **16 levels in an L8 buffer** gives the full saving. The `image` crate can't write 4-bit anyway.
  - Typst holds an L8 crop at 1 B/px, against 3 B/px for RGB.
- **Photos** (`annotate.rs`): `REPORT_PHOTO_PX = 800` and `REPORT_JPEG_QUALITY = 80` are already constants. `report_photo` embeds a photo that is small enough and has no shapes byte for byte.
  - The stroke is a fraction of the long side, so at 640 px the shapes print just as thick (4.8 px ≈ 0.6 mm at 80 mm).
- **Windows today:** the plugin's `desktop.rs` `view_pdf` returns `Unsupported`, which reaches the user as `internal`. Neither `tauri-plugin-opener` nor the `open` crate is in `Cargo.lock`. `tauri-plugin-dialog` 2.7.3 is there (rfd), with a Rust API (`blocking_save_file`).
- **The lifecycle gap:**
  - The single-flight slot (`commands/report.rs`) lives as long as the Rust run.
  - The card's "Generating…" lives only in `jobs.ts` module state.
  - If the page reloads while the process lives, the card shows the old line with Generate enabled. Generate then answers `report_busy`, and the run's result is dropped: its callback id is gone (tauri `scripts/core.js`).
  - Ways to get there: Back on Projects finishes the activity while the process lives (D-016), or an activity recreation.
  - Nothing in the app, wry 0.55.1 or Tauri handles `onRenderProcessGone`, so a dead renderer takes the whole app down.
  - Leftover `.part` files and extra PDFs of a killed run stay until the project's next successful generation, because the sweep skips live projects.
  - The sweep runs in `setup` before `AppState` is managed (`lib.rs`), so no run can be in progress while it runs.
- **Version:** `0.0.1` appears in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` (+ `Cargo.lock`) and `package.json`. versionName reaches the APK through the generated `tauri.properties`. CI derives the APK name from `tauri.conf.json`.

## 1. Windows: Open and "Save a copy…" (same card, no Share)
- **Open** = the default PDF app, through the plugin's `desktop.rs` `view_pdf` with the **`open` crate** (`open::that_detached`, `ShellExecuteW` on Windows). It is a desktop-only dependency of the plugin, so the APK doesn't change.
  - `open_report` stays the same on every platform. `tauri-plugin-opener` would be the same crate plus JS bindings, permissions and an Android library; it is not used.
  - Errors: Windows "no association" (`ERROR_NO_ASSOCIATION`, 1155) → a desktop `Error::NoApp`, which `rejected_code()` maps to `no_app` and then to `no_pdf_app`. Any other error → `internal` with the OS message.
  - It works on the Mac dev build too (Preview), which helps the local check.
- **The copy Open hands over gets a folder per report:** `<app cache>/reports/<project id>/<stamp>/<readable name>`, where `<stamp>` comes from `meta.file`.
  - If that copy already exists as a regular file with `meta.bytes`, it is reused without copying.
  - This matters on Windows: Acrobat locks an open PDF, so with 6a's single `<pid>/<name>` a second Open would fail its rename and show "File system error". A new report gets a new folder, so a locked old copy never blocks it.
  - On Android Share uses the same layout, and a repeated tap no longer copies 18 MB.
  - The purge goes one level deeper: files older than 24 h, then empty folders. 6a's flat `<pid>/<name>` copies are purged by age.
  - Delete still removes `<pid>/` whole. The Kotlin check is a canonical-path `startsWith(cacheDir/reports/)` (`CameraCapturePlugin.kt`), so nested folders pass and **Kotlin doesn't change**. FileProvider's `cache-path reports/` covers subfolders.
- **Save a copy…** → new command `save_report_copy(project_id, title) -> Option<String>` (the saved file name; `None` when the dialog is cancelled). **The WebView still never passes a path.**
  - Rust opens the dialog with `app.dialog().file().set_parent(window).set_title(title).set_file_name(meta.name).add_filter("PDF", ["pdf"]).set_directory(Documents).blocking_save_file()`, inside `spawn_blocking`.
  - The dialog asks before overwriting. If the chosen name lacks `.pdf`, it is added; checked first against what rfd's Windows dialog does itself.
  - Core `reports::save_copy(data_dir, pid, dest)`: `latest()`, copy to `<dest>.part`, `sync_all`, rename over `dest`, remove the part on error.
  - An I/O error → new code **`save_failed`**: "Could not save the copy. If a file with that name is open in another app, close it and try again." PT: "Não foi possível guardar a cópia. Se um ficheiro com esse nome estiver aberto noutra aplicação, feche-o e tente de novo."
  - On Android the dialog returns a URI rather than a path, which is refused; the card never shows the button there.
- **Card:**
  - Android: Open + Share (unchanged).
  - Desktop: **Open** + **"Save a copy…"** ("Guardar uma cópia…"), both disabled while generating or handing over.
  - After a save, a muted line "Saved: <file name>" ("Guardado: …"), cleared by the next Generate.
- **i18n:** `report.save_copy`, `report.save_title`, `report.saved`, `error.save_failed`, plus ErrorBanner `save_failed`.

### Windows in CI (no Windows machine before the preview)
- **`ci.yml` windows job:** the "Report tests" step becomes `cargo test -p checkflat-core -p checkflat-report`.
  - This is the core's first Windows run. A test that is genuinely Unix-only (symlinks through `std::os::unix`, permission bits) gets `#[cfg(unix)]`, and D-029 lists it.
  - Anything else that fails is a real Windows defect and gets fixed.
- **Lock tests** in core `reports` (and `tests/` where they fit):
  - **Simulating a held file.** std's `File::open` on Windows shares read, write and delete, so a plain handle locks nothing there. The tests hold the file with `OpenOptionsExt::share_mode(FILE_SHARE_READ)` under `#[cfg(windows)]`, which blocks delete and rename the way Acrobat does. On Unix they hold a plain handle, where an open file never blocks a rename.
  - **`save_copy` over an existing file** (all platforms): replaced, with no `.part` left. *Windows:* the destination held open → `Err`, the destination unchanged, no `.part` left.
  - **`share_copy` (= Open) with an earlier copy still held open:**
    - the same report → the held copy is reused (`Ok`, same path, not rewritten)
    - a new report → its own folder, `Ok`
    - a held copy past 24 h → the purge logs a warning and doesn't fail; after release the next purge removes it
  - **Regenerate + prune while the stored report or an older share copy is held open:**
    - `write_report` succeeds and `latest()` names the new PDF
    - the held old PDF survives the prune (warning)
    - after release, the next `write_report` prunes it
    - a held share copy has no effect on generation
- **D-029 records what only CI covers on Windows:** lock and rename semantics, and the core/report tests. It also records what nothing covers before the preview: `ShellExecuteW` open, the rfd save dialog, WebView2 mouse/keyboard, the installer and SmartScreen.

## 2. Smaller report images (constants, measured current vs smaller)
- **`snapshot.rs`:**
  - **Crops are always L8** (`imageops::grayscale`), quantised to `CROP_GREY_LEVELS: u8 = 16`: `v → round(v·15/255)·17`, so 255 stays 255 and near-white rounds to white.
  - The overview keeps its encoding (the chroma rule, 256 levels); it is one per plan.
  - `GREY_CHROMA_MAX`/`is_grey` stay for the overview only. `Snapshot.grey` is documented as such.
- **`annotate.rs`:** `REPORT_PHOTO_PX = 640`, `REPORT_JPEG_QUALITY = 75`. A stored photo ≤ 640 px without shapes is still embedded as stored.
- **Tests:**
  - `tests/snapshot.rs`: the crop asserts take the quantised grey. A coloured fixture crop now expects L8 luma. The overview asserts stay exact.
  - `annotate.rs`: the 800×600 pixel probes are rewritten as fractions of the output size (stroke ≈ 4.8 px at 640).
  - `tests/report.rs`: 640×480.
  - New: a crop uses at most 16 grey values; the overview keeps colour.
- **The decision is Martín's, from the print.** The values are constants. If B's print isn't acceptable, he names what to relax (e.g. 32 levels, or 800 px at q75), and that variant is measured the same way.

### P30 measurement — "Perf A1 4a" (50 × 3, annotated, client's A1 plan), plan viewed first
**Builds.** Both are `VITE_SPIKES=1 pnpm tauri android build --apk --target aarch64 --features dev-seed -c '{"bundle":{"android":{"versionCode":N}}}'`, universal APK, CI key, `adb install -r`, data kept. N = the installed versionCode, read first with `dumpsys package com.checkflat.app | grep versionCode` (67 or later after 6a).
- **A "current"** = the branch right after the plan commit (= `main` code, 800 px / q80, chroma-rule crops).
- **B "smaller"** = A + the image commit only, so the comparison isolates the images.

**Samplers.**
- `spikes/pss.sh -s <serial> 1.5`: app + renderer, ≈ 2 s in practice.
- `spikes/pss-app-fast.sh`, an on-device loop of `dumpsys meminfo <pid>` for the app process at ≈ 8 samples/s, to catch the 1–2 s spike that 1.5 s sampling misses (D-027). This is 4b's ad-hoc loop, now committed.

**Runs:** A, B, A, B (alternating, so drift doesn't favour one), each driven by adb with the same script, as 4b was (720×1560 tap space):
1. `adb install -r <A or B>`, `am force-stop`, `am start -n com.checkflat.app/.MainActivity`, wait 5 s.
2. Tap "Perf A1 4a" → open the A1 plan → wait for it → `KEYCODE_PLUS` ×3 (≈ 3.4×) → 4 swipes → Back to the project screen.
3. Language PT. Start both samplers, wait 5 s, tap Generate.
4. Poll `Documents/` every 1 s for the new PDF (the dev-seed copy). When it appears, wait 10 s, then stop the samplers.
5. Pull the PDF and `dev.log` (the timing line: total, snapshots, photos, compile, pdf).

Each run takes ≈ 2 min. USB drops every 15–17 min, so each window holds two runs, with `adb wait-for-device` in between. The session starts only on Martín's "go"; he only needs the phone on USB, unlocked, with the screen kept on.

**Recorded per run:**
- total time and the four stages
- peak total PSS (pss.sh), with app and renderer
- the fast app peak + the renderer at that moment
- pages and bytes
- per image type from `pdfimages -list`: crops (count, encoding, bytes) and photos (size, bytes)

**Pass criteria for B:**
- **peak total PSS ≤ 600 MB in both runs**, by either sampler (≥ 50 MB under the 650 MB limit)
- time ≤ 60 s (expected ≈ 20 s)
- PDF ≤ 15 MB (expected ≈ 10–11 MB, from D-024's ratios)

A is the reference (expected ≈ 640 MB, 18.4 MB, ≈ 21 s). **If B misses the memory criterion, work stops and Martín sees the numbers before any fallback** (chunked compile).

**Samples for printing** (on Martín's Desktop; both from the P30 runs, with the same layout and page numbers, checked with `pdftotext`):
- `Checkflat 6b sample A - current (800px q80, colour crops).pdf`
- `Checkflat 6b sample B - smaller (640px q75, 16 grey levels).pdf`

They come with 3–4 page numbers to print from each:
- a crop that had colour (trees, street fill)
- dense line work with a pin near an edge
- annotated landscape photos
- a portrait photo

Martín sees the numbers before anything is committed.

## 3. App switch during generation
**What must hold** (written into D-029):
- **H1. No half-written report.** After a kill at any moment, the card's line and Open/Share/Save name a complete PDF: the previous one, or the new one (`latest()` checks the size). **New:** the startup sweep also removes live projects' `<pid>-*.pdf.part` and `<pid>.json.part`, plus PDFs their metadata doesn't name. This is safe because the sweep runs before any command is served; tests in `tests/sweep.rs`.
- **H2. The card recovers; nothing gets stuck on "Generating".**
  - (a) The process survived: on return the card shows "Generating…" until the run ends, then the new line.
  - (b) The process was killed: after the relaunch, the previous line (or "no report yet") with Generate enabled.
  - (c) The page reloaded while the process kept running: "Generating…" with the run's language, then the new line.
- **H3. Generate works again:** at once after a kill, and as soon as the run ends otherwise. The card never offers a Generate that answers `report_busy` for its own project.

**Expected from the code: (c) fails today**, through Back → Back exit → relaunch, or an activity recreation. **The simple fix is built only after the emulator confirms it:**
- The slot keeps `{project id, lang}`, and a new command `report_running() -> Option<{projectId, lang}>`.
- `jobs.ts` gets a pure `followBackend(projectId, {running, sleep}, signal)`: it polls every 1 s while this project's run is going, and stops when it ends or the card unmounts.
- The card asks it on mount when no JS job exists, then calls `latestReport`.
- Tests: follows to the end, stops on abort, ignores another project's run.

**Tests:**
- **Emulator** (`Phone_API35`, Android 15, debug build; it covers the cached-app freezer and gives `run-as` for the files):
  - **E1:** Generate → Home → 30 s in Settings → return. Was the process frozen (`dumpsys activity processes`)? Did the run advance? Card state.
  - **E2:** Generate → Back → Back (activity finished, process alive) → relaunch within 10 s → card state. This is the (c) case, run before and after the fix.
  - **E3:** Generate → Home → `am kill com.checkflat.app` mid-run → relaunch. Previous line; Open shows all its pages; Generate works; `run-as ls reports/` shows no `.part` and only the named PDF + JSON.
  - **E4:** `settings put global always_finish_activities 1` ("Don't keep activities") → Generate → Home → return. Same check as E2; reset to 0 afterwards.
- **P30** (the CI build of the PR head with any fix, "Perf A1 4a"; driven by adb, with `pidof` before and after and `logcat -b events` for `am_kill` / `am_proc_died`):
  - **P1:** Generate (PT) → open another app (Chrome) for 30 s → back through recents. Did the process survive? Card state; the new line; Open.
  - **P2:** **Martín sets** Developer options → Background process limit → "No background processes". Then Generate → another app 30 s → return. Expected: the process was killed; previous line, no "Generating", Generate completes, Open shows the new report. **He resets it to "Standard limit"** afterwards.
  - **P3:** Generate → Back → Back (exit) → relaunch from the launcher → "Generating…", then the line.

**Pass:** H1–H3 in every case. A kill costs the run but never data. If H1–H3 cannot hold with this path, **a foreground-service proposal goes to Martín; it is not built unasked.** If P1 shows EMUI killing the app in normal use, D-029 records it, with what it means for HyperOS: the user regenerates in the foreground (≈ 20 s on the P30).

**Added during 6b (Martín, after the P30 check, choice b):** Back on the Projects screen finishes the activity, and the process ends a few seconds later on Android 10 and 15, cancelling a run (P3, E2). While a run is in progress, Back at the root now sends the app to the background, as Home does (the plugin's `moveTaskToBack`). With no run it closes the app as before (D-016). Rechecked on the emulator and on the P30 (CI build 77). P2 also needed a second app: the limit exempts the previous app (D-029).

## 4. Version 0.1.0 and the build number on both platforms
- `0.1.0` in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` (+ `Cargo.lock`) and `package.json`.
- **Build number on Windows:** the windows job's "Build NSIS installer" step gets `env: CHECKFLAT_BUILD: ${{ github.run_number }}`.
  - The plugin's desktop `app_version` returns `version_code: option_env!("CHECKFLAT_BUILD").and_then(|s| s.parse().ok())`. rustc tracks `option_env!`, so cargo rebuilds when the value changes.
  - Local builds have no value and show the version only.
  - Android is unchanged: PackageManager's versionCode is the same CI run number (D-021).
- The About line (no change to the card):
  - Android and CI Windows: "Versão 0.1.0 (build N)"
  - local builds: "Versão 0.1.0"
  - "· debug" as before
- CI's APK name becomes `checkflat-0.1.0-N-arm64.apk`; NSIS gives `Checkflat_0.1.0_x64-setup.exe`.

## 5. Windows checklists (run by the friend on his laptop during the preview, after merge)
### 5a. Technical reference (in D-029; results "pending preview")
Install the installer of the merged build. SmartScreen: More info → Run anyway (the installer is unsigned). App data is in `%APPDATA%\com.checkflat.app`, share copies in `%LOCALAPPDATA%\com.checkflat.app\reports\`.
1. **About:** the Projects screen shows "Version 0.1.0 (build N)", N = the CI run.
2. **Plan from file:** new project → import a PDF plan (A1 if possible). Tiles are made, the plan opens, and it pans and zooms with the mouse wheel and drag.
3. **Photo from file, save, restart (the D-022 test):**
   - "+" → click the plan → sheet → Gallery: the file picker shows images only. Pick a landscape JPEG and a portrait one, plus a PNG; all appear upright.
   - Description → Save; the pin shows.
   - Close the app (window ✕), start it again: the pin, the photos and the description are there, and so is the list row with its thumbnail.
4. **Annotation by mouse:**
   - Open a photo's editor: ellipse (drag), arrow (tail → head), freehand. The hint reads "Draw with the mouse. Click a shape to select it."
   - A click on a shape selects it (halo), Delete removes it, Undo, Done.
   - Save → reopen: the shapes are where they were drawn.
   - Escape in the editor closes the editor only; the sheet stays.
5. **Keyboard with the sheet and the editor open:**
   - Sheet open: Tab and Shift+Tab cycle inside the sheet only; the top bar, the plan, the pin bar's Cancel and the side list are never reached.
   - Editor open: Tab and Shift+Tab stay in the editor.
   - Escape order: editor first, then the sheet.
   - The 1e sequence: open a create sheet, Shift+Tab a dozen times, then Escape; Move on a pin afterwards shows the move bar, not a new-observation sheet.
6. **Mouse path, briefly:** move mode (select → Move → drag → Confirm; Cancel restores); the observation list → a row → the plan on its pin; Back to the list.
7. **Report, Generate:** EN, then PT. The bar shows and the controls are disabled; then the line with date, language, pages and size. **No Share button.**
8. **Open:** the default PDF app (Edge, Acrobat, …) opens "<project> - <date> - PT.pdf", all pages and photos right. With the viewer still open, Open again → it opens again with no error. Generate EN while the old one is still open → works; Open → the EN file.
9. **Save a copy…:**
   - The dialog proposes "<project> - <date> - EN.pdf" in Documents. Save to the Desktop → "Saved: …"; the file opens.
   - Again to the same name → the dialog asks to replace → Yes → replaced.
   - Cancel the dialog → nothing happens, no message.
   - If Acrobat is installed: with that Desktop file open in Acrobat, save over it → "Could not save the copy… close it and try again".
10. **Restart:** the latest line is still there, and Open works.
11. **Delete a test project:** its folder under `%LOCALAPPDATA%\com.checkflat.app\reports\` is gone.

**Pass:** 1–11 as described (9's Acrobat case only if Acrobat is there). The results are added to D-029 in a later docs-only commit, from the tester's report.

### 5b. Short PT guide for the tester
Plain text for his guide (no Tab sequences, no AppData folders, no Acrobat-lock case). It goes to Martín as plain text when the PR is open.

### 5c. The annotation hint matches the input
- `AnnotationEditor.svelte` shows `annotate.hint` ("Draw with one finger. Tap a shape to select it.") in the hint line and on the stage's `aria-label`.
- With the card's platform check (`isAndroid()` from `platform.ts`), desktop shows a new **`annotate.hint_mouse`**: "Draw with the mouse. Click a shape to select it." / "Desenhe com o rato. Clique numa forma para a selecionar.". Android is unchanged.
- The `annotate.limit` hint is platform-neutral and stays.
- Key parity is guarded by `en_and_pt_have_identical_keys`.

## 6. Docs
- This section and the header comment: the **first commit**.
- **D-029:**
  - context and options: `open` vs the opener plugin; Rust-side dialog vs a JS path; folder per report vs reuse by name; 4-bit PNG vs L8 at 16 levels
  - outcome, with the P30 A/B table and Martín's print choice
  - H1–H3 with the E/P results
  - Windows: what only CI covers and what nothing covers before the preview; the technical checklist with its results **"pending preview"**
  - open items
- **CHANGELOG** "Sprint 6b".
- **`checkflatPLAN.md`:**
  - §3 rows: snapshots (16-level grey crops), Images/Report (640 px / q75), hand-off (Windows: Open with the default app, Save a copy…)
  - §4 Sprint 6b ✅ and v0.1
  - the pre-v0.1 Windows test line: now run by the friend during the preview, with CI covering file locking

## 7. Added during 6b: photos two per row (Martín, 2026-10-26)
After choosing the smaller images (print choice B), Martín decided the report's photos go **two per row** instead of one below the other, for a more compact report; this replaces the single column of the 2026-10-13 answers (D-025). A 2-column grid on the plan row's columns; each photo keeps its maximum of 80 mm wide and 70 mm tall with its caption under it; three photos are a row of two and a row of one; a row is never split across pages. Done in `assets/report.typ`; `tests/photo_layout.rs` pins it from the PDF's content stream. Images are unchanged, so there is no P30 re-measure; the Mac harness gives the page counts, time and size (D-029).

## Steps (commit per step)
1. `git checkout -b sprint-6b fc362f1`, then this plan commit. **Build A** (dev-seed) and keep it in the scratchpad.
2. **Images:** constants, crops, tests; `spikes/pss-app-fast.sh`. A Mac release-harness run of `measure_50_observations` on the fixture, before and after, as a sanity check. **Push and open a draft PR** (CI runs on PRs only).
3. **Build B** → P30 A/B session on Martín's "go" → numbers and Desktop samples to him → he prints and chooses. The rest continues meanwhile; a different choice is one later commit plus a re-measure.
4. **Lifecycle:** E1–E4 on a debug build first. Then the sweep step (H1), `report_running` + `followBackend` + the card (H2c), with tests; then E2/E3/E4 again.
5. **Windows:**
   - core `share_copy` folder per report + reuse + purge, and `save_copy`
   - the lock tests (§1)
   - `ci.yml`: core + report tests on the windows job
   - plugin desktop `view_pdf` (`open`) + `Error::NoApp`
   - `save_report_copy` command + registration
   - `api.ts`, card, i18n, ErrorBanner
   - the annotation hint per platform (§5c)
   - **Mac dev check** (also the mouse hint in the editor): Open (Preview) and Save a copy (NSSavePanel) on a cover-only report, since the Mac can't import plans
   - the `cfg(windows)` tests run first in CI, and CI is iterated until they pass
6. **Version 0.1.0 + `CHECKFLAT_BUILD`** in the windows job.
7. CI green, then the P30 session on the CI build:
   - P1–P3
   - quick regression by touch or adb: About line 0.1.0, Open, Share once (new copy path), how the smaller report looks
8. Docs: D-029 (Windows results "pending preview"), CHANGELOG, plan §3/§4. PR ready → **stop** (the super review is Martín's to set up). The PT tester guide goes to him as plain text.
9. *After the merge, not blocking:* the friend's Windows run during the preview → his results go into D-029 in a docs-only commit.

## Files touched
- **Report crate:** `crates/checkflat-report/src/{snapshot.rs, annotate.rs}`, `assets/report.typ` (photos two per row), `tests/{snapshot.rs, report.rs, photo_layout.rs (new)}`
- **Core:** `crates/checkflat-core/src/reports.rs` (share layout, purge, `save_copy`, live-leftover sweep), `tests/{sweep.rs, …}`
- **Tauri:** `src-tauri/src/commands/report.rs` (slot `{pid, lang}`, `report_running`, `save_report_copy`), `src-tauri/src/lib.rs` (handlers), `src-tauri/Cargo.toml`, `tauri.conf.json`
- **Plugin:** `src-tauri/plugins/camera-capture/{Cargo.toml (desktop `open`), src/desktop.rs (`view_pdf`, `app_version` build), src/error.rs, README.md}`
- **CI:** `.github/workflows/ci.yml` (windows job: core + report tests, `CHECKFLAT_BUILD`)
- **Frontend:** `src/lib/report/{ReportCard.svelte, jobs.ts, jobs.test.ts}`, `src/lib/annotate/AnnotationEditor.svelte`, `src/lib/api.ts`, `src/lib/components/ErrorBanner.svelte`, `i18n/{en,pt}.json`, `package.json`
- **Tools:** `spikes/pss-app-fast.sh` (new)
- **Docs:** `docs/sprint-6-plan.md`, `docs/decisions.md` (D-029), `docs/checkflatPLAN.md`, `CHANGELOG.md`

## Verification
- `pnpm check && pnpm test`; `cargo test --workspace --all-features`; `cargo clippy --workspace --all-features --all-targets -- -D warnings`.
- CI green on all three:
  - checks
  - android-release: the dev-seed and FileProvider guards, the APK size line, and an APK name carrying 0.1.0
  - windows: the core + report tests including the `cfg(windows)` lock tests, and the installer built with `CHECKFLAT_BUILD`
- The P30 A/B session, E1–E4 and P1–P3, as above. Results go into D-029 only after Martín has seen the numbers and given his words. The Windows checklist runs after the merge (pending preview).
