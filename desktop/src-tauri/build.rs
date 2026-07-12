// build.rs — 仅在 tauri_app feature 启用时跑 tauri-build。
// E1：注入 BUILD_DATE 给 `env!("BUILD_DATE")`，供关于页显示。
fn main() {
    println!(
        "cargo:rustc-env=BUILD_DATE={}",
        chrono::Utc::now().format("%Y-%m-%d")
    );
    #[cfg(feature = "tauri_app")]
    {
        tauri_build::build()
    }
}
