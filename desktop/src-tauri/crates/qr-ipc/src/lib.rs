//! 主进程与 `qr_helper.exe`（elevated）之间的共享协议。
//!
//! 纯 serde 类型，零平台依赖：主进程（`helper_client.rs`）与 helper
//! （`src/bin/qr_helper.rs`）各自实现传输层，帧格式统一为
//! 「4 字节小端长度前缀 + JSON 报文」。
//!
//! 安全约束（display.md §4.2）：
//! - helper 只接受本文件枚举内的固定指令，**不接受任意注册表路径/命令行**；
//! - 写操作目标由 helper 根据 `MonitorKey` 自行推导注册表路径；
//! - `Handshake` 必须携带主进程生成的 nonce（经计划任务 `$(Arg0)` 传入）。

use serde::{Deserialize, Serialize};

/// 协议版本。helper 与主程序不一致即拒绝服务（触发重新 --install）。
pub const PROTOCOL_VERSION: u32 = 1;

/// 命名管道名（不含 `\\.\pipe\` 前缀）。
pub const PIPE_NAME: &str = "soundlink.qrhelper";
/// 计划任务名。
pub const TASK_NAME: &str = "SoundLink QR Helper";
/// 帧长度上限（防御性，正常报文 << 1MB；EDID 最大 64KB）。
pub const MAX_FRAME_BYTES: u32 = 4 * 1024 * 1024;
/// helper 空闲自动退出时长（秒）。
pub const IDLE_EXIT_SECS: u64 = 300;

/// 显示器稳定主键（重启/换口不丢）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MonitorKey {
    /// 显示器设备实例路径（如 `DISPLAY\LGS1234\5&2F3A...`），唯一且持久。
    pub instance_path: String,
    /// EDID SHA-256 前 16 字节 hex（换显示器同口可区分）。
    pub edid_hash: String,
}

impl MonitorKey {
    /// 展示用短 id（备份文件名等）。
    pub fn short(&self) -> String {
        let mut s = String::with_capacity(8);
        for c in self.edid_hash.chars().take(8) {
            s.push(c);
        }
        s
    }
}

/// 设备重启目标。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestartTarget {
    /// 仅显示器设备（代价最小）。
    Monitor,
    /// 显示适配器（影响全部屏幕，约 3 秒黑屏）。
    Adapter,
}

/// EDID Override 写入的注册表变体（探测得出，不硬编码）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegVariant {
    /// `HKLM\SYSTEM\...\Enum\DISPLAY\<id>\<inst>\Device Parameters\EDID_OVERRIDE`
    MonitorInstanceOverride,
    /// `HKLM\SYSTEM\...\Control\Class\{4d36e96e-...}\NNNN\EDID_OVERRIDE`
    ClassMonitorOverride,
    /// `HKLM\SYSTEM\...\Control\GraphicsDrivers\Configuration\<...>\00 (+\00)`
    GraphicsDriversConfiguration,
}

/// 让 override 生效的激活方式（按代价升序探测）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationMethod {
    MonitorRestart,
    AdapterRestart,
    /// 均失败：需注销/重启系统（本期仅提示，不自动做）。
    LogoffRequired,
}

/// 自适应探测计划（helper 侧执行写+重启，主进程侧编排验证）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbePlan {
    pub monitor: MonitorKey,
    /// 无害探针 EDID（已含等价 timing 副本），用于验证注册表变体。
    pub probe_edid: Vec<u8>,
    /// 依次尝试的注册表变体。
    pub variants: Vec<RegVariant>,
    /// 备份 id（写 override 前强制备份）。
    pub backup_id: String,
}

/// 主进程 -> helper 请求。
#[derive(Debug, Serialize, Deserialize)]
pub enum HelperRequest {
    /// 握手：nonce + 客户端版本。必须为首帧，否则断开。
    Handshake { nonce: [u8; 32], client_version: String },
    /// 读取当前生效 EDID（注册表 override 优先，不存在则读显示器原生）。
    ReadEdid { monitor: MonitorKey },
    /// 写入 EDID Override（写前自动备份原值）。
    WriteEdidOverride { monitor: MonitorKey, edid: Vec<u8>, backup_id: String, variant: RegVariant },
    /// 删除 EDID Override（恢复显示器原生 EDID）。
    RemoveEdidOverride { monitor: MonitorKey, variant: RegVariant },
    /// 重启显示器/适配器设备（SetupAPI 禁启用）。
    RestartDevice { target: RestartTarget, monitor: MonitorKey },
    /// 武装看门狗：seconds 内未 DisarmWatchdog 则自动还原 backup_id 对应 EDID 并重启设备。
    ArmWatchdog { seconds: u32, backup_id: String, monitor: MonitorKey, variant: RegVariant },
    /// 解除看门狗。
    DisarmWatchdog,
    /// 按备份 id 还原某显示器 EDID（不重启设备）。
    RestoreEdid { backup_id: String, monitor: MonitorKey, variant: RegVariant },
    /// 探测：写入无害探针 EDID（由主进程随后验证系统模式列表变化）。
    Probe { plan: ProbePlan },
}

/// helper -> 主进程响应。
#[derive(Debug, Serialize, Deserialize)]
pub enum HelperResponse {
    Ok,
    HandshakeOk { helper_version: String, protocol: u32 },
    Edid(Vec<u8>),
    Written { variant: RegVariant, backup_path: String },
    Restarted { method: ActivationMethod, elapsed_ms: u64 },
    Err { code: HelperErrCode, msg: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HelperErrCode {
    /// 首帧不是 Handshake 或 nonce 不匹配。
    BadHandshake,
    /// 客户端进程校验失败（路径/签名不符）。
    UntrustedClient,
    /// 协议/版本不匹配。
    VersionMismatch,
    /// 注册表读写失败。
    Registry,
    /// 设备重启失败。
    RestartFailed,
    /// 备份/还原失败。
    Backup,
    /// 请求参数非法。
    BadRequest,
    /// helper 内部错误。
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> MonitorKey {
        MonitorKey {
            instance_path: "DISPLAY\\LGS1234\\5&2F3A".into(),
            edid_hash: "0123456789abcdef".into(),
        }
    }

    #[test]
    fn request_roundtrip() {
        let req = HelperRequest::WriteEdidOverride {
            monitor: key(),
            edid: vec![0u8; 256],
            backup_id: "b1".into(),
            variant: RegVariant::MonitorInstanceOverride,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: HelperRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, HelperRequest::WriteEdidOverride { .. }));
    }

    #[test]
    fn response_roundtrip() {
        let resp = HelperResponse::Err { code: HelperErrCode::BadHandshake, msg: "x".into() };
        let json = serde_json::to_string(&resp).unwrap();
        let back: HelperResponse = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, HelperResponse::Err { code: HelperErrCode::BadHandshake, .. }));
    }

    #[test]
    fn monitor_key_short() {
        assert_eq!(key().short(), "01234567");
    }
}
