<!-- Approved 2026-10-22 (plan mode, Claude Code), with Martín's decisions: copy to cache/reports/ on Open/Share, remove the template's FileProvider, a CI guard on the APK's FileProvider, PT labels "Abrir"/"Partilhar". The implementation record is docs/decisions.md D-028. Part 6b is planned when it starts. -->

# Sprint 6 — report Open/Share (6a), Windows save and smaller report images (6b) (plan)

Part **6a** is planned in full below. Part **6b** (Windows share/save, smaller report images — 16-level greyscale crops and 640 px / q75 photos, D-024/D-026/D-027 —, the HyperOS app-switch check, the end-of-Sprint-6 super review with the deferred Windows exit test) gets its own plan section when it starts, on a branch `sprint-6b` from `main` after 6a merges.

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
