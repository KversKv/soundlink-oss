// main.rs — 占位（Tauri 2 桌面端入口）
//
// 职责：初始化 tracing 日志、加载配置、启动 Rust Core（网络/音频/发现），
// 注册 Tauri commands（见 commands/），运行 Tauri 应用。
//
// 模块划分：
//   mod commands;  // 暴露给前端的命令
//   mod audio;     // jitter_buffer / opus_decoder / resampler / output
//   mod network;   // discovery / udp_receiver / control_server / packet
//   mod pairing;   // pairing_code / key_exchange / trust_store
//   mod device;    // audio_device / device_identity
//   mod config;    // 配置读写 (SQLite/JSON)
//   mod logging;   // tracing 初始化
//
// 注意：进入阶段 1 时由 `tauri init` 生成的脚手架整合本文件。
// 详见 docs/First/02-architecture.md

fn main() {
    // TODO: 阶段 1 实现
}
