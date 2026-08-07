//! SoundLink 桌面核心库。
//!
//! 不依赖 Tauri，可独立编译与单测；被 `main.rs`（Tauri 应用）与
//! `examples/loopback_sender.rs`（自测）复用。对齐 docs/First/11-implementation-spec.md。

pub mod audio;
pub mod commands;
pub mod config;
pub mod constants;
pub mod device;
pub mod license;
pub mod logging;
pub mod network;
pub mod pairing;
pub mod receiver;
pub mod sender;

pub use receiver::{ReceiverEngine, ReceiverStatus};
pub use sender::{SenderEngine, SenderStatus};
