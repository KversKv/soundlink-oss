//! QR-1：elevated 辅助进程（display.md §四）。
//!
//! 命令行：
//! - `qr_helper --install`        一次性注册计划任务（requireAdministrator manifest 触发 UAC）
//! - `qr_helper --uninstall`      删除计划任务
//! - `qr_helper --serve <nonce>`  由计划任务拉起：命名管道服务（最高权限）
//! - `qr_helper --restore-all`    安全模式救援：还原全部 EDID 备份并删除计划任务
//!
//! 安全（§4.2）：管道 ACL 仅当前用户 + SYSTEM；nonce 握手；客户端签名校验；
//! 命令白名单；版本绑定；5 分钟空闲退出；写操作全量审计日志。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use soundlink_lib::features::quick_resolution::helper_core;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = helper_core::run(&args);
    std::process::exit(code);
}
