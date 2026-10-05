# Checkflat
Site-visit report app: PDF plans, pinned observations, photos, PDF reports. Tauri v2 (Rust), Android + Windows.
See `docs/checkflatPLAN.md` (plan, section 0 = client answers) and `docs/decisions.md` (architecture decisions).

## Status
Sprint 0 (foundations + risk spikes). The app is a spike shell with three tabs: Plan (PDF.js A1 viewer), Camera, Report (Typst vs printpdf). No product code yet.

## Dev setup (macOS host)
Tools: Node ≥ 24, pnpm 12, Rust stable (≥ 1.85), Temurin JDK 21, Android SDK (platform 35, build-tools 35, NDK 27), Android emulator.

```sh
brew install --cask temurin@21 android-commandlinetools
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
# ~/.zshrc
export JAVA_HOME="$(/usr/libexec/java_home -v 21)"
export ANDROID_HOME="/opt/homebrew/share/android-commandlinetools"
export NDK_HOME="$ANDROID_HOME/ndk/27.2.12479018"
export PATH="$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$PATH"
# SDK packages (pick the system image ABI for your host: `uname -m` → arm64 = arm64-v8a, x86_64 = x86_64)
sdkmanager --licenses
sdkmanager "platform-tools" "emulator" "platforms;android-35" "build-tools;35.0.0" "ndk;27.2.12479018" \
           "system-images;android-35;google_apis;arm64-v8a"
```

AVDs used for the spikes (create with `avdmanager create avd -n <name> -k "system-images;android-35;google_apis;arm64-v8a" -d <profile>`):

| AVD | profile | screen | notes |
|---|---|---|---|
| `Phone_API35` | pixel_8 | 1080×2400 @420dpi | `hw.camera.back=virtualscene`, `hw.gpu.enabled=yes` in `config.ini` |
| `Tablet_API35` | pixel_tablet | 2560×1600 @320dpi | same |

`hw.camera.back=virtualscene` gives the camera intent a fake 3D scene, so photo capture can be tested without hardware.

## Client data
`Examples/` (client plans) is gitignored and must never be committed; `public/spike/` stays ignored as a safety net. Plans are tested the way the client uses them: import through the app. On an emulator or phone, put the PDF where the Android picker finds it:

```sh
adb push Examples/c-JS00-AM01.pdf /sdcard/Download/   # then Import plan → ☰ → Downloads
```
The first open renders the plan's tile pyramid once (D-014); afterwards it opens instantly.

## Data directory
Tauri app data dir (`com.checkflat.app`): `checkflat.db` (+ `-wal`, `-shm`, `.bak-v<n>` before migrations), `projects/<project_id>/plans/<plan_id>.pdf`, `projects/.trash/` (quarantine, purged after 7 days), `tmp/` (import staging). All DB paths are relative to this dir. macOS dev: `~/Library/Application Support/com.checkflat.app`.

## Commands
```sh
pnpm install                  # also copies PDF.js cmaps/standard_fonts/wasm/iccs into public/pdfjs/
pnpm tauri dev                # desktop window on the host (macOS is not a target, just the fastest loop)
pnpm tauri android dev        # on the running emulator / connected phone
pnpm tauri android build --debug --apk --target aarch64   # local debug APK
cargo test -p report-spike --features typst,printpdf      # Spike C engine tests; PDFs land in spikes/out/
pnpm tauri android build --apk --target aarch64 -c '{"build":{"features":["report-typst"]}}'  # single-engine release APK (size only)
spikes/measure.sh             # Spike C clean/incremental build times per engine
adb shell dumpsys meminfo com.checkflat.app               # memory numbers for Spike A
```
Windows installers are built only in CI (`.github/workflows/ci.yml`).
