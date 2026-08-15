const COMMANDS: &[&str] = &[
    "speak",
    "stop",
    "get_voices",
    "is_speaking",
    "is_initialized",
    "pause_speaking",
    "resume_speaking",
    "preview_voice",
    "register_listener",
    "set_background_behavior",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
