const COMMANDS: &[&str] = &["capture", "pick_image", "display_name"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
