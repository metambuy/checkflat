const COMMANDS: &[&str] = &["capture", "display_name"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
