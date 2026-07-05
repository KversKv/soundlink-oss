//! mDNS 广播 _soundlink._udp.local（阶段 3 完整实现）。
//! 阶段 1：占位 + 最小广播，供后续接入。

use crate::constants::{DEFAULT_AUDIO_PORT, DEFAULT_CONTROL_PORT, MDNS_SERVICE_TYPE};

/// 阶段 1 占位：返回广播服务类型与端口信息（实际 mDNS 注册见阶段 3）。
pub fn discovery_info() -> (&'static str, u16, u16) {
    (MDNS_SERVICE_TYPE, DEFAULT_CONTROL_PORT, DEFAULT_AUDIO_PORT)
}
