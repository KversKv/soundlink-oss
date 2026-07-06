//! mDNS 广播 `_soundlink._udp.local`（阶段 3）。
//!
//! 使用 `mdns-sd` 注册服务，TXT 记录对齐 `docs/First/04-protocol.md` §2。
//! 移动端通过 Bonjour/NSD/`multicast_dns` 发现。

use crate::constants::{
    DEFAULT_AUDIO_PORT, DEFAULT_CONTROL_PORT, MDNS_SERVICE_TYPE, PROTOCOL_VERSION,
};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use parking_lot::Mutex;
use std::net::IpAddr;

/// mDNS 广播器：注册 `_soundlink._udp.local.` 服务。
pub struct MdnsBroadcaster {
    daemon: Mutex<Option<ServiceDaemon>>,
    fullname: Mutex<Option<String>>,
}

impl Default for MdnsBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl MdnsBroadcaster {
    pub fn new() -> Self {
        Self {
            daemon: Mutex::new(None),
            fullname: Mutex::new(None),
        }
    }

    /// 启动广播。`ip` 为 None 时自动检测本机 IP。
    pub fn start(
        &self,
        device_id: &str,
        device_name: &str,
        ip: Option<IpAddr>,
        control_port: u16,
        audio_port: u16,
        pairing_required: bool,
    ) -> Result<(), String> {
        if self.daemon.lock().is_some() {
            return Err("mDNS 广播已在运行".into());
        }
        let daemon = ServiceDaemon::new().map_err(|e| format!("创建 mDNS daemon 失败：{}", e))?;

        // 实例名 = device_id（保证唯一）。
        let instance = device_id;
        let host_name = format!("{}.local.", device_id.replace('.', "-"));
        let ip_str = ip
            .map(|a| a.to_string())
            .unwrap_or_else(|| "127.0.0.1".to_string());

        let props: Vec<(&str, String)> = vec![
            ("device_id", device_id.to_string()),
            ("device_name", device_name.to_string()),
            ("role", "receiver".to_string()),
            ("protocol_version", PROTOCOL_VERSION.to_string()),
            ("pairing_required", pairing_required.to_string()),
            ("audio_codec", "opus".to_string()),
            ("sample_rate", crate::constants::SAMPLE_RATE.to_string()),
            ("control_port", control_port.to_string()),
            ("audio_port", audio_port.to_string()),
        ];

        let mut info = ServiceInfo::new(
            MDNS_SERVICE_TYPE,
            instance,
            &host_name,
            &ip_str,
            audio_port,
            &props[..],
        )
        .map_err(|e| format!("构造 ServiceInfo 失败：{}", e))?;

        // 无显式 IP 时启用自动地址检测。
        if ip.is_none() {
            info = info.enable_addr_auto();
        }

        let fullname = info.get_fullname().to_string();
        daemon
            .register(info)
            .map_err(|e| format!("注册 mDNS 服务失败：{}", e))?;

        *self.fullname.lock() = Some(fullname);
        *self.daemon.lock() = Some(daemon);
        Ok(())
    }

    /// 停止广播。
    pub fn stop(&self) {
        let daemon = self.daemon.lock().take();
        let fullname = self.fullname.lock().take();
        if let (Some(d), Some(f)) = (daemon, fullname) {
            let _ = d.unregister(&f);
            let _ = d.shutdown();
        }
    }

    pub fn is_running(&self) -> bool {
        self.daemon.lock().is_some()
    }
}

/// 返回广播服务类型与端口信息（保留阶段 1 接口兼容）。
pub fn discovery_info() -> (&'static str, u16, u16) {
    (MDNS_SERVICE_TYPE, DEFAULT_CONTROL_PORT, DEFAULT_AUDIO_PORT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_stop_no_crash() {
        // 基本启停不崩溃（无网络环境可能创建失败，跳过）。
        let b = MdnsBroadcaster::new();
        let r = b.start("pc-test", "Test PC", None, 47810, 47811, true);
        if r.is_ok() {
            b.stop();
        }
    }
}
