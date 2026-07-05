//! tracing 日志初始化（控制台）。禁止记录密钥/配对码。
//! 文件日志后续补；级别由环境变量 `SOUNDLINK_LOG` 控制，默认 info。

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init() {
    let filter =
        EnvFilter::try_from_env("SOUNDLINK_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(filter)
        .try_init();
}
