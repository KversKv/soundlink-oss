// build.rs — 仅在 tauri_app feature 启用时跑 tauri-build。
fn main() {
    #[cfg(feature = "tauri_app")]
    {
        tauri_build::build()
    }
}
