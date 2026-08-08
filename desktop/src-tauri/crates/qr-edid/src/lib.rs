//! EDID 解析/编辑/校验 + timing 计算（display.md §15 `qr-edid`）。
//!
//! 纯逻辑 crate，零 Windows 依赖，可完整单测。
//! - [`parse`]：base block + CTA-861 + DisplayID 2.0 解析
//! - [`edit`]：DTD 槽位管理、DisplayID 2.0 扩展块追加、checksum
//! - [`timing`]：CVT-RB v2/v3、native-blanking 继承
//!
//! 关键事实：EDID DTD 的像素时钟字段为 16bit×10kHz，上限 655.35MHz；
//! 高刷新模式（如 1920×1440@480Hz ≈ 1.41GHz）必须走 DisplayID 2.0
//! Type VII（20 字节描述符，24bit×10kHz 像素时钟）。

pub mod edit;
pub mod parse;
pub mod timing;

pub use edit::{EdidDoc, Slot};
pub use parse::EdidInfo;
pub use timing::{generate, TimingParams, TimingStandard};

/// EDID/编辑错误。
#[derive(Debug, thiserror::Error)]
pub enum EdidErr {
    #[error("EDID 长度不足（{0} 字节，至少 128）")]
    TooShort(usize),
    #[error("EDID 头签名无效")]
    BadHeader,
    #[error("第 {block} 块 checksum 无效")]
    BadChecksum { block: usize },
    #[error("EDID 无可用时序槽位")]
    NoSlot,
    #[error("扩展块数量已达上限（255）")]
    TooManyExtensions,
    #[error("像素时钟 {0}kHz 超出 DTD 上限 655350kHz，需 DisplayID 槽位")]
    PixelClockTooHighForDtd(u32),
    #[error("timing 参数非法：{0}")]
    BadTiming(&'static str),
}

/// EDID 固定头。
pub(crate) const EDID_HEADER: [u8; 8] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
/// 单块字节数。
pub(crate) const BLOCK_SIZE: usize = 128;
/// DTD 可表达的最大像素时钟（kHz）：0xFFFF × 10kHz。
pub const DTD_MAX_PIXEL_CLOCK_KHZ: u32 = 655_350;
/// CTA-861 扩展 tag。
pub(crate) const EXT_TAG_CTA: u8 = 0x02;
/// DisplayID 扩展 tag。
pub(crate) const EXT_TAG_DISPLAYID: u8 = 0x70;
/// DisplayID 2.0 Type VII timing 数据块 tag。
pub(crate) const DISPLAYID_BLOCK_TYPE_VII: u8 = 0x13;
