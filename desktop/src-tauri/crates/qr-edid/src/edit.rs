//! EDID 编辑：DTD 槽位管理 + DisplayID 2.0 扩展块 + checksum。
//!
//! 安全约束（display.md §7.3/§十八-10）：
//! - 只覆写「dummy（tag 0x10）/全零」描述符槽与 CTA DTD 区零填充，
//!   **绝不覆盖** 0xFC/0xFD/0xFF 等显示器描述符与既有 DTD；
//! - 像素时钟 > 655.35MHz 的 timing 只能进 DisplayID 2.0 Type VII；
//! - 每次编辑后重算受影响块的 checksum 与 base 块扩展计数。

use crate::parse;
use crate::timing::TimingParams;
use crate::{
    EdidErr, BLOCK_SIZE, DISPLAYID_BLOCK_TYPE_VII, DTD_MAX_PIXEL_CLOCK_KHZ, EXT_TAG_CTA,
    EXT_TAG_DISPLAYID,
};

/// timing 写入位置（返回给调用方记录）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// base block 第 N 个描述符槽（0..4）。
    BaseDtd(u8),
    /// 第 ext 个 CTA 扩展块内 DTD 区偏移（字节）。
    CtaDtd { ext: u8, off: u8 },
    /// 第 ext 个 DisplayID 扩展内第 idx 条 Type VII 描述符。
    DisplayIdTypeVii { ext: u8, idx: u8 },
}

/// 可编辑 EDID 文档（已校验）。
#[derive(Debug, Clone)]
pub struct EdidDoc {
    bytes: Vec<u8>,
}

/// base block 描述符槽偏移。
const BASE_DESC_OFFSETS: [usize; 4] = [0x36, 0x48, 0x5A, 0x6C];
/// 显示器描述符 tag：dummy（可被覆写）。
const DESC_TAG_DUMMY: u8 = 0x10;
/// DisplayID 2.0 单扩展内数据区可用上限（[5..127)，留 2 字节余量）。
const DISPLAYID_DATA_MAX: usize = 120;

impl EdidDoc {
    /// 校验并加载（头 + 全块 checksum）。
    pub fn parse(bytes: &[u8]) -> Result<Self, EdidErr> {
        parse::parse(bytes)?;
        Ok(Self { bytes: bytes.to_vec() })
    }

    /// 解析信息（转发 [`parse::parse`]）。
    pub fn info(&self) -> parse::EdidInfo {
        parse::parse(&self.bytes).expect("EdidDoc 已校验，不应解析失败")
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    /// 扩展块数量（base[126]）。
    pub fn extension_count(&self) -> u8 {
        self.bytes[126]
    }

    /// 空闲 DTD 槽位数（base dummy/零槽 + CTA DTD 区零填充槽）。
    pub fn free_dtd_slots(&self) -> u8 {
        let mut n = 0u8;
        for &off in &BASE_DESC_OFFSETS {
            if self.base_slot_free(off) {
                n += 1;
            }
        }
        for ext in self.extensions_of(EXT_TAG_CTA) {
            let block = &self.bytes[ext * BLOCK_SIZE..(ext + 1) * BLOCK_SIZE];
            n += cta_free_slots(block);
        }
        n
    }

    /// DisplayID Type VII 剩余描述符容量（条）。
    pub fn displayid_descriptor_capacity(&self) -> u8 {
        let mut cap = 0u8;
        for ext in self.extensions_of(EXT_TAG_DISPLAYID) {
            let block = &self.bytes[ext * BLOCK_SIZE..(ext + 1) * BLOCK_SIZE];
            if block[1] < 0x20 {
                continue; // 仅 DisplayID 2.x
            }
            let used = block[2] as usize; // section size
            // 无 Type VII 块时不计（由 append_displayid_block 创建带空块的扩展）。
            if find_type_vii(block).is_some() {
                let free = DISPLAYID_DATA_MAX.saturating_sub(used);
                cap = cap.saturating_add((free / 20) as u8);
            }
        }
        cap
    }

    /// 最大可追加扩展块数（EDID 上限 255 块含 base）。
    pub fn max_extension_blocks(&self) -> u8 {
        255u8.saturating_sub(self.extension_count())
    }

    /// 插入一条 timing。优先 DTD 槽；像素时钟超 DTD 上限或无 DTD 槽时走 DisplayID。
    ///
    /// 返回 [`EdidErr::NoSlot`] 时调用方应 [`Self::append_displayid_block`] 后重试。
    pub fn insert_timing(&mut self, t: &TimingParams, refresh_hz: u32) -> Result<Slot, EdidErr> {
        let clock_khz = t.pixel_clock_khz(refresh_hz);
        if clock_khz == 0 {
            return Err(EdidErr::BadTiming("像素时钟为 0"));
        }
        if clock_khz <= DTD_MAX_PIXEL_CLOCK_KHZ {
            // 1) base dummy/零槽
            for (i, &off) in BASE_DESC_OFFSETS.iter().enumerate() {
                if self.base_slot_free(off) {
                    let dtd = encode_dtd(t, clock_khz)?;
                    self.bytes[off..off + 18].copy_from_slice(&dtd);
                    self.fix_checksum(0);
                    return Ok(Slot::BaseDtd(i as u8));
                }
            }
            // 2) CTA DTD 区零填充槽
            let exts = self.extensions_of(EXT_TAG_CTA);
            for ext in exts {
                let base = ext * BLOCK_SIZE;
                let dtd_offset = self.bytes[base + 2] as usize;
                if dtd_offset < 4 {
                    continue;
                }
                let mut off = dtd_offset;
                while off + 18 <= BLOCK_SIZE - 1 {
                    let abs = base + off;
                    let d = &self.bytes[abs..abs + 18];
                    if d[0] == 0 && d[1] == 0 {
                        // 零填充：要求整槽全零（避免误伤 CTA 短描述符区）。
                        if d.iter().all(|&b| b == 0) {
                            let dtd = encode_dtd(t, clock_khz)?;
                            self.bytes[abs..abs + 18].copy_from_slice(&dtd);
                            self.fix_checksum(ext);
                            return Ok(Slot::CtaDtd { ext: ext as u8, off: off as u8 });
                        }
                        break; // 非全零 = DTD 区结束（后续为 pad）。
                    }
                    off += 18;
                }
            }
        }
        // 3) DisplayID Type VII
        let exts = self.extensions_of(EXT_TAG_DISPLAYID);
        for ext in exts {
            let base = ext * BLOCK_SIZE;
            if self.bytes[base + 1] < 0x20 {
                continue;
            }
            let block_copy: Vec<u8> = self.bytes[base..base + BLOCK_SIZE].to_vec();
            if let Some((blk_off, payload_off, payload_len)) = find_type_vii(&block_copy) {
                let used = block_copy[2] as usize;
                if used + 20 > DISPLAYID_DATA_MAX {
                    continue;
                }
                let desc = encode_displayid_type_vii(t, clock_khz)?;
                let dst = base + payload_off + payload_len;
                self.bytes[dst..dst + 20].copy_from_slice(&desc);
                // 更新块长、section size。
                self.bytes[base + blk_off + 2] = (payload_len + 20) as u8;
                self.bytes[base + 2] = (used + 20) as u8;
                self.fix_checksum(ext);
                let idx = (payload_len / 20) as u8;
                return Ok(Slot::DisplayIdTypeVii { ext: ext as u8, idx });
            }
        }
        Err(EdidErr::NoSlot)
    }

    /// 追加一个 DisplayID 2.0 扩展块（含空 Type VII 块，供后续插入）。
    pub fn append_displayid_block(&mut self) -> Result<(), EdidErr> {
        if self.max_extension_blocks() == 0 {
            return Err(EdidErr::TooManyExtensions);
        }
        let mut block = [0u8; BLOCK_SIZE];
        block[0] = EXT_TAG_DISPLAYID;
        block[1] = 0x20; // DisplayID 2.0
        // 空 Type VII 块（tag/ver/len=0）占 3 字节。
        block[2] = 3; // section size
        block[3] = 0x03; // product type: standalone display
        block[4] = 0x00; // extension count
        block[5] = DISPLAYID_BLOCK_TYPE_VII;
        block[6] = 0x00;
        block[7] = 0x00;
        self.bytes.extend_from_slice(&block);
        self.fix_extension_count();
        let last = self.bytes.len() / BLOCK_SIZE - 1;
        self.fix_checksum(last);
        Ok(())
    }

    /// 修正 base 块扩展计数 = 实际扩展块数。
    pub fn fix_extension_count(&mut self) {
        let n = (self.bytes.len() / BLOCK_SIZE).saturating_sub(1);
        self.bytes[126] = n as u8;
        self.fix_checksum(0);
    }

    /// 重算所有块 checksum。
    pub fn recompute_all_checksums(&mut self) {
        let n = self.bytes.len() / BLOCK_SIZE;
        for i in 0..n {
            self.fix_checksum(i);
        }
    }

    // ---- 内部 ----

    fn fix_checksum(&mut self, block_idx: usize) {
        let start = block_idx * BLOCK_SIZE;
        let end = start + BLOCK_SIZE;
        if end > self.bytes.len() {
            return;
        }
        let cks = parse::checksum_byte(&self.bytes[start..end - 1]);
        self.bytes[end - 1] = cks;
    }

    /// base 槽是否可覆写（dummy 或全零，且不是既有 DTD）。
    fn base_slot_free(&self, off: usize) -> bool {
        let d = &self.bytes[off..off + 18];
        if d[0] != 0 || d[1] != 0 {
            return false; // 既有 DTD。
        }
        // 显示器描述符：仅 dummy(0x10)/全零(0x00) 可覆写。
        matches!(d[3], DESC_TAG_DUMMY | 0x00)
    }

    /// 扩展块序号列表（1-based 块号）。
    fn extensions_of(&self, tag: u8) -> Vec<usize> {
        let n = self.bytes.len() / BLOCK_SIZE;
        (1..n)
            .filter(|&i| self.bytes[i * BLOCK_SIZE] == tag)
            .collect()
    }
}

/// CTA 块内 DTD 区零填充槽计数。
fn cta_free_slots(block: &[u8]) -> u8 {
    let dtd_offset = block[2] as usize;
    if dtd_offset < 4 {
        return 0;
    }
    let mut n = 0;
    let mut off = dtd_offset;
    while off + 18 <= BLOCK_SIZE - 1 {
        let d = &block[off..off + 18];
        if d[0] == 0 && d[1] == 0 {
            if d.iter().all(|&b| b == 0) {
                n += 1;
                off += 18;
                continue;
            }
            break;
        }
        off += 18;
    }
    n
}

/// 在 DisplayID 2.x 块中定位 Type VII 数据块：
/// 返回 (块头偏移, payload 偏移, payload 长度)。
fn find_type_vii(block: &[u8]) -> Option<(usize, usize, usize)> {
    if block[1] < 0x20 {
        return None;
    }
    let section_end = 5 + block[2] as usize;
    let mut i = 5usize;
    while i + 3 <= section_end.min(BLOCK_SIZE - 1) {
        let tag = block[i];
        let len = block[i + 2] as usize;
        if tag == DISPLAYID_BLOCK_TYPE_VII {
            return Some((i, i + 3, len));
        }
        if len == 0 {
            i += 3;
        } else {
            i += 3 + len;
        }
    }
    None
}

/// 18 字节 DTD 编码。字段宽度校验：active/blank ≤4095，fp/sync H ≤1023，V ≤63。
fn encode_dtd(t: &TimingParams, clock_khz: u32) -> Result<[u8; 18], EdidErr> {
    let pix10 = clock_khz / 10;
    if pix10 > 0xFFFF {
        return Err(EdidErr::PixelClockTooHighForDtd(clock_khz));
    }
    if t.h_active > 4095 || t.v_active > 4095 || t.h_blank() > 4095 || t.v_blank() > 4095 {
        return Err(EdidErr::BadTiming("active/blank 超 12bit"));
    }
    if t.h_front > 1023 || t.h_sync > 1023 || t.v_front > 63 || t.v_sync > 63 {
        return Err(EdidErr::BadTiming("fp/sync 超位宽"));
    }
    let h_blank = t.h_blank();
    let v_blank = t.v_blank();
    let mut d = [0u8; 18];
    d[0] = (pix10 & 0xFF) as u8;
    d[1] = (pix10 >> 8) as u8;
    d[2] = t.h_active as u8;
    d[3] = h_blank as u8;
    d[4] = (((t.h_active >> 8) as u8) << 4) | ((h_blank >> 8) as u8);
    d[5] = t.v_active as u8;
    d[6] = v_blank as u8;
    d[7] = (((t.v_active >> 8) as u8) << 4) | ((v_blank >> 8) as u8);
    d[8] = t.h_front as u8;
    d[9] = t.h_sync as u8;
    d[10] = (((t.v_front & 0xF) as u8) << 4) | (t.v_sync & 0xF) as u8;
    d[11] = ((((t.h_front >> 8) & 0x3) as u8) << 6)
        | ((((t.h_sync >> 8) & 0x3) as u8) << 4)
        | ((((t.v_front >> 4) & 0x3) as u8) << 2)
        | (((t.v_sync >> 4) & 0x3) as u8);
    // d[12..14] 图像尺寸 mm：0 = 未指定。
    let mut flags = 0x18u8; // 数字分离同步
    if t.h_sync_pol {
        flags |= 0x02;
    }
    if t.v_sync_pol {
        flags |= 0x04;
    }
    if t.interlaced {
        flags |= 0x80;
    }
    d[17] = flags;
    Ok(d)
}

/// DisplayID 2.0 Type VII 20 字节描述符编码。
fn encode_displayid_type_vii(t: &TimingParams, clock_khz: u32) -> Result<[u8; 20], EdidErr> {
    if t.h_active > 65535 || t.v_active > 65535 {
        return Err(EdidErr::BadTiming("active 超 16bit"));
    }
    let pix10 = clock_khz / 10 + 1; // 存储值 = 实际值 + 1
    if pix10 > 0xFF_FFFF {
        return Err(EdidErr::BadTiming("像素时钟超 24bit"));
    }
    let mut d = [0u8; 20];
    d[0] = (pix10 & 0xFF) as u8;
    d[1] = ((pix10 >> 8) & 0xFF) as u8;
    d[2] = ((pix10 >> 16) & 0xFF) as u8;
    d[3] = aspect_code(t.h_active, t.v_active); // preferred/stereo 均为 0
    let put16 = |d: &mut [u8; 20], off: usize, v: u32| {
        d[off] = (v & 0xFF) as u8;
        d[off + 1] = ((v >> 8) & 0xFF) as u8;
    };
    put16(&mut d, 4, t.h_active + 1);
    put16(&mut d, 6, t.h_blank() + 1);
    put16(&mut d, 8, (t.h_front + 1) & 0x7FFF);
    let hsync_w = ((t.h_sync + 1) & 0x7FFF) | if t.h_sync_pol { 0x8000 } else { 0 };
    put16(&mut d, 10, hsync_w);
    put16(&mut d, 12, t.v_active + 1);
    put16(&mut d, 14, t.v_blank() + 1);
    put16(&mut d, 16, (t.v_front + 1) & 0x7FFF);
    let vsync_w = ((t.v_sync + 1) & 0x7F) as u8 | if t.v_sync_pol { 0x80 } else { 0 };
    d[18] = vsync_w;
    d[19] = 0;
    Ok(d)
}

/// 最接近的 DisplayID 宽高比代码（装饰性元数据）。
fn aspect_code(h: u32, v: u32) -> u8 {
    if v == 0 {
        return 0;
    }
    let r = h as f32 / v as f32;
    // (code, ratio)：1:1, 5:4, 4:3, 15:9, 16:9, 16:10, 64:10, 32:9
    const CODES: [(u8, f32); 8] = [
        (0, 1.0),
        (1, 5.0 / 4.0),
        (2, 4.0 / 3.0),
        (3, 15.0 / 9.0),
        (4, 16.0 / 9.0),
        (5, 16.0 / 10.0),
        (6, 64.0 / 10.0),
        (7, 32.0 / 9.0),
    ];
    CODES.iter()
        .min_by(|a, b| (r - a.1).abs().partial_cmp(&(r - b.1).abs()).unwrap())
        .map(|(c, _)| *c)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timing::{generate, TimingStandard};
    use crate::EDID_HEADER;

    /// 基础 fixture：base DTD 占用 1 槽，其余 3 槽 dummy(0x10)。
    fn fixture_with_dummy_slots() -> Vec<u8> {
        let mut e = vec![0u8; 128];
        e[0..8].copy_from_slice(&EDID_HEADER);
        e[8] = 0x04;
        e[9] = 0x43;
        // 槽0：真实 DTD（1920×1080@60，148.5MHz 标准）
        let native = TimingParams {
            h_active: 1920,
            v_active: 1080,
            h_front: 88,
            h_sync: 44,
            h_back: 148,
            v_front: 4,
            v_sync: 5,
            v_back: 36,
            h_sync_pol: true,
            v_sync_pol: true,
            interlaced: false,
        };
        let dtd = encode_dtd(&native, 148_500).unwrap();
        e[0x36..0x36 + 18].copy_from_slice(&dtd);
        // 槽1..3：dummy 0x10
        for &off in &[0x48usize, 0x5A, 0x6C] {
            e[off + 3] = DESC_TAG_DUMMY;
        }
        e[126] = 0;
        let cks = parse::checksum_byte(&e[..127]);
        e[127] = cks;
        e
    }

    /// 全部 4 槽被真实 DTD 占用的 fixture。
    fn fixture_full_base() -> Vec<u8> {
        let mut e = fixture_with_dummy_slots();
        let native = TimingParams {
            h_active: 1280,
            v_active: 720,
            h_front: 110,
            h_sync: 40,
            h_back: 220,
            v_front: 5,
            v_sync: 5,
            v_back: 20,
            h_sync_pol: true,
            v_sync_pol: true,
            interlaced: false,
        };
        let dtd = encode_dtd(&native, 74_250).unwrap();
        for &off in &[0x48usize, 0x5A, 0x6C] {
            e[off..off + 18].copy_from_slice(&dtd);
        }
        let cks = parse::checksum_byte(&e[..127]);
        e[127] = cks;
        e
    }

    #[test]
    fn free_slots_counted() {
        let doc = EdidDoc::parse(&fixture_with_dummy_slots()).unwrap();
        assert_eq!(doc.free_dtd_slots(), 3);
        let doc2 = EdidDoc::parse(&fixture_full_base()).unwrap();
        assert_eq!(doc2.free_dtd_slots(), 0);
    }

    #[test]
    fn insert_into_dummy_slot_roundtrip() {
        let mut doc = EdidDoc::parse(&fixture_with_dummy_slots()).unwrap();
        let t = generate(TimingStandard::CvtRb2, 1920, 1440, 144, None).unwrap();
        let slot = doc.insert_timing(&t, 144).unwrap();
        assert_eq!(slot, Slot::BaseDtd(1));
        assert_eq!(doc.free_dtd_slots(), 2);
        // 重解析：2 条 detailed timing，checksum 有效。
        let info = doc.info();
        assert_eq!(info.detailed_timings.len(), 2);
        let inserted = &info.detailed_timings[1];
        assert_eq!(inserted.h_active, 1920);
        assert_eq!(inserted.v_active, 1440);
        assert_eq!(inserted.h_total(), t.h_total());
        assert_eq!(inserted.v_total(), t.v_total());
    }

    #[test]
    fn insert_never_overwrites_real_descriptors() {
        // 全满 base：name/range-limits 场景由 parse fixture 覆盖；
        // 这里验证 DTD 占用全部槽时插入返回 NoSlot（无 DisplayID）。
        let mut doc = EdidDoc::parse(&fixture_full_base()).unwrap();
        let t = generate(TimingStandard::CvtRb2, 1600, 1200, 120, None).unwrap();
        assert!(matches!(doc.insert_timing(&t, 120), Err(EdidErr::NoSlot)));
    }

    #[test]
    fn append_displayid_then_insert() {
        let mut doc = EdidDoc::parse(&fixture_full_base()).unwrap();
        doc.append_displayid_block().unwrap();
        assert_eq!(doc.extension_count(), 1);
        assert!(doc.displayid_descriptor_capacity() >= 5);
        let t = generate(TimingStandard::CvtRb2, 1600, 1200, 120, None).unwrap();
        let slot = doc.insert_timing(&t, 120).unwrap();
        assert_eq!(slot, Slot::DisplayIdTypeVii { ext: 1, idx: 0 });
        // 再插一条进入同一扩展块 idx=1。
        let t2 = generate(TimingStandard::CvtRb2, 1440, 1080, 144, None).unwrap();
        let slot2 = doc.insert_timing(&t2, 144).unwrap();
        assert_eq!(slot2, Slot::DisplayIdTypeVii { ext: 1, idx: 1 });
        // checksum 全块有效（info() 内部会校验）。
        let info = doc.info();
        assert!(info.displayid_supported);
        assert_eq!(info.extension_tags, vec![EXT_TAG_DISPLAYID]);
    }

    #[test]
    fn high_pixel_clock_skips_dtd() {
        // 1920×1440@480：像素时钟 ~1.4GHz > 655.35MHz → 即使有空 DTD 槽也必须走 DisplayID。
        let mut doc = EdidDoc::parse(&fixture_with_dummy_slots()).unwrap();
        doc.append_displayid_block().unwrap();
        let t = generate(TimingStandard::CvtRb3, 1920, 1440, 480, None).unwrap();
        let clock = t.pixel_clock_khz(480);
        assert!(clock > DTD_MAX_PIXEL_CLOCK_KHZ, "clock={clock}");
        let slot = doc.insert_timing(&t, 480).unwrap();
        assert!(matches!(slot, Slot::DisplayIdTypeVii { .. }));
    }

    #[test]
    fn high_pixel_clock_without_displayid_is_no_slot() {
        let mut doc = EdidDoc::parse(&fixture_with_dummy_slots()).unwrap();
        let t = generate(TimingStandard::CvtRb3, 1920, 1440, 480, None).unwrap();
        assert!(matches!(doc.insert_timing(&t, 480), Err(EdidErr::NoSlot)));
    }

    #[test]
    fn dtd_encode_decode_roundtrip() {
        let t = TimingParams {
            h_active: 2560,
            v_active: 1440,
            h_front: 8,
            h_sync: 32,
            h_back: 40,
            v_front: 3,
            v_sync: 5,
            v_back: 33,
            h_sync_pol: false,
            v_sync_pol: true,
            interlaced: false,
        };
        let d = encode_dtd(&t, 241_500).unwrap();
        let back = parse::decode_dtd(&d);
        assert_eq!(back.h_active, 2560);
        assert_eq!(back.h_blank(), t.h_blank());
        assert_eq!(back.v_active, 1440);
        assert_eq!(back.v_blank(), t.v_blank());
        assert_eq!(back.h_front, 8);
        assert_eq!(back.h_sync, 32);
        assert_eq!(back.v_front, 3);
        assert_eq!(back.v_sync, 5);
        assert!(!back.h_sync_pol);
        assert!(back.v_sync_pol);
    }

    #[test]
    fn aspect_codes() {
        assert_eq!(aspect_code(1920, 1080), 4); // 16:9
        assert_eq!(aspect_code(1920, 1440), 2); // 4:3
        assert_eq!(aspect_code(1920, 1200), 5); // 16:10
    }
}
