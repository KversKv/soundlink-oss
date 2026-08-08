//! qr_helper 核心逻辑（lib 内实现，bin 仅转发，便于单测与复用）。

pub mod audit;
pub mod pipe_server;
pub mod scheduled_task;
pub mod watchdog;

/// 主入口：解析命令行并分发。返回进程退出码。
pub fn run(args: &[String]) -> i32 {
    let cmd = args.get(1).map(|s| s.as_str());
    match cmd {
        Some("--install") => match scheduled_task::install() {
            Ok(()) => {
                eprintln!("[qr_helper] 计划任务注册成功");
                0
            }
            Err(e) => {
                eprintln!("[qr_helper] 计划任务注册失败：{}", e);
                3
            }
        },
        Some("--uninstall") => match scheduled_task::uninstall() {
            Ok(()) => {
                eprintln!("[qr_helper] 计划任务已删除");
                0
            }
            Err(e) => {
                eprintln!("[qr_helper] 计划任务删除失败：{}", e);
                3
            }
        },
        Some("--serve") => {
            // nonce 由主进程写入临时文件（计划任务 /Run 无法携带参数）。
            let nonce = match read_nonce_file() {
                Some(n) => n,
                None => {
                    eprintln!("[qr_helper] --serve 缺少 nonce 文件（%APPDATA%/soundlink/qr_nonce.tmp）");
                    return 2;
                }
            };
            match pipe_server::serve(nonce) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("[qr_helper] serve 失败：{}", e);
                    4
                }
            }
        }
        Some("--restore-all") => match watchdog::restore_all() {
            Ok(n) => {
                eprintln!("[qr_helper] 已还原 {} 份 EDID 备份", n);
                0
            }
            Err(e) => {
                eprintln!("[qr_helper] restore-all 失败：{}", e);
                5
            }
        },
        _ => {
            eprintln!("用法: qr_helper --install | --uninstall | --serve <nonce_hex> | --restore-all");
            2
        }
    }
}

fn parse_nonce(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// 读取 nonce 临时文件（读后删除，防复用）。
fn read_nonce_file() -> Option<[u8; 32]> {
    let path = crate::features::quick_resolution::platform::windows::helper_client::nonce_file_path();
    let text = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    parse_nonce(text.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_parse_ok() {
        let hex = "ab".repeat(32);
        let n = parse_nonce(&hex).unwrap();
        assert_eq!(n[0], 0xab);
        assert_eq!(n[31], 0xab);
    }

    #[test]
    fn nonce_parse_rejects() {
        assert!(parse_nonce("").is_none());
        assert!(parse_nonce(&"zz".repeat(32)).is_none());
        assert!(parse_nonce(&"ab".repeat(31)).is_none());
    }
}
