//! tracing 日志初始化：控制台 + 按日滚动文件日志（E4）。
//! 禁止记录密钥/配对码。级别由环境变量 `SOUNDLINK_LOG` 控制，默认 info。
//!
//! 文件日志路径：`%APPDATA%/soundlink/logs/soundlink-YYYY-MM-DD.log`。
//! 不依赖 tracing-appender（避免拉取额外 crate），用 std::fs 直接追加写入。
//! 文件日志仅在 `tauri_app` feature 下启用（依赖 `dirs` 定位配置目录）。

use std::path::PathBuf;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// E4：日志目录（`%APPDATA%/soundlink/logs/`）。返回 None 表示无法定位配置目录。
#[cfg(feature = "tauri_app")]
pub fn log_dir() -> Option<PathBuf> {
    let mut p = dirs::config_dir()?;
    p.push("soundlink");
    p.push("logs");
    Some(p)
}

/// 非 tauri_app 构建下：始终返回 None（仅控制台日志）。
#[cfg(not(feature = "tauri_app"))]
pub fn log_dir() -> Option<PathBuf> {
    None
}

pub fn init() {
    let filter =
        EnvFilter::try_from_env("SOUNDLINK_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    let registry = tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(filter);

    // E4：仅 tauri_app 构建下尝试启用按日滚动文件日志。
    #[cfg(feature = "tauri_app")]
    {
        if let Some(log_dir) = log_dir() {
            if std::fs::create_dir_all(&log_dir).is_ok() {
                let writer = DailyFileWriter::new(log_dir);
                let file_layer = fmt::layer()
                    .with_ansi(false)
                    .with_writer(std::sync::Mutex::new(writer));
                let _ = registry.with(file_layer).try_init();
                return;
            }
        }
    }

    let _ = registry.try_init();
}

// ─────────────────── E4：按日滚动的文件 writer ───────────────────

#[cfg(feature = "tauri_app")]
mod daily_writer {
    use std::fs::{File, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;

    /// E4：当前日期字符串 `YYYY-MM-DD`（UTC）。
    fn today_date() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let days = (secs / 86_400) as i64;
        let (y, m, d) = days_to_ymd(days);
        format!("{:04}-{:02}-{:02}", y, m, d)
    }

    /// 将「自 1970-01-01 起的天数」转成 (year, month, day)。仅 UTC。
    /// 算法来源：https://howardhinnant.github.io/date_algorithms.html（civil_from_days）。
    fn days_to_ymd(z: i64) -> (i64, u32, u32) {
        let z = z + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = (z - era * 146_097) as u64; // [0, 146_096]
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
        let mp = (5 * doy + 2) / 153; // [0, 11]
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
        (if m <= 2 { y + 1 } else { y }, m, d)
    }

    /// E4：当日日志文件路径。
    fn today_log_path(log_dir: &PathBuf) -> PathBuf {
        log_dir.join(format!("soundlink-{}.log", today_date()))
    }

    /// E4：按日滚动的文件 writer。检测日期变化时切换文件。
    pub(super) struct DailyFileWriter {
        dir: PathBuf,
        current_date: String,
        file: Option<File>,
    }

    impl DailyFileWriter {
        pub(super) fn new(dir: PathBuf) -> Self {
            Self {
                dir,
                current_date: String::new(),
                file: None,
            }
        }

        fn ensure_open(&mut self) -> std::io::Result<&mut File> {
            let today = today_date();
            if today != self.current_date || self.file.is_none() {
                let _ = std::fs::create_dir_all(&self.dir);
                let path = today_log_path(&self.dir);
                let f = OpenOptions::new().create(true).append(true).open(&path)?;
                self.file = Some(f);
                self.current_date = today;
            }
            Ok(self.file.as_mut().unwrap())
        }
    }

    impl Write for DailyFileWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let f = self.ensure_open()?;
            let n = f.write(buf)?;
            let _ = f.flush();
            Ok(n)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            if let Some(f) = self.file.as_mut() {
                f.flush()?;
            }
            Ok(())
        }
    }
}

#[cfg(feature = "tauri_app")]
use daily_writer::DailyFileWriter;
