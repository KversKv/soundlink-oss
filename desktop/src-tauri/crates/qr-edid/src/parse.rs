//! EDID 解析：base block + CTA-861 + DisplayID 2.0。
//!
//! 只解析本功能需要的字段（display.md §5/§6）：
//! 厂商/型号/名称、详细 timing（DTD）、像素时钟上限（Display Range Limits）、
//! DSC 能力线索（CTA HDMI Forum VSDB / DisplayID 2.0 存在性）。

use crate::timing::TimingParams;
use crate::{EdidErr, BLOCK_SIZE, EDID_HEADER, EXT_TAG_CTA, EXT_TAG_DISPLAYID};

/// 解析结论（只含本功能消费的信息）。
#[derive(Debug, Clone, Default)]
pub struct EdidInfo {
    /// 3 字母厂商代码（如 "LGS"）。
    pub manufacturer: String,
    pub product_code: u16,
    pub serial: u32,
    /// 显示器名称描述符（tag 0xFC）。
    pub name: Option<String>,
    /// Display Range Limits 上报的最大像素时钟（kHz）。
    pub max_pixel_clock_khz: Option<u32>,
    /// Display Range Limits 上报的最大行频（kHz）——驱动按此裁剪高刷自定义模式。
    pub max_h_freq_khz: Option<u32>,
    /// Display Range Limits 上报的最大场频（Hz）。
    pub max_v_rate_hz: Option<u32>,
    /// 全部详细 timing（按出现顺序；首个通常即原生模式）。
    pub detailed_timings: Vec<TimingParams>,
    /// DTD 对应的刷新率估计（kHz 像素时钟 / total）。
    pub detailed_refresh_hz: Vec<u32>,
    /// 扩展块 tag 列表（0x02=CTA-861, 0x70=DisplayID...）。
    pub extension_tags: Vec<u8>,
    /// 是否存在 DisplayID 2.x 扩展（追加 timing 的承载）。
    pub displayid_supported: bool,
    /// CTA HDMI Forum VSDB 中的 DSC 能力线索（None = 未声明）。
    pub dsc_hdmi_forum: Option<bool>,
}

/// 校验并解析完整 EDID（长度必须是 128 的倍数）。
pub fn parse(edid: &[u8]) -> Result<EdidInfo, EdidErr> {
    if edid.len() < BLOCK_SIZE || edid.len() % BLOCK_SIZE != 0 {
        return Err(EdidErr::TooShort(edid.len()));
    }
    if edid[0..8] != EDID_HEADER {
        return Err(EdidErr::BadHeader);
    }
    for (i, block) in edid.chunks(BLOCK_SIZE).enumerate() {
        if checksum(block) != 0 {
            return Err(EdidErr::BadChecksum { block: i });
        }
    }

    let mut info = EdidInfo::default();
    parse_base(&edid[..BLOCK_SIZE], &mut info);

    let ext_count = edid[126] as usize;
    for i in 1..=ext_count.min(edid.len() / BLOCK_SIZE - 1) {
        let block = &edid[i * BLOCK_SIZE..(i + 1) * BLOCK_SIZE];
        let tag = block[0];
        info.extension_tags.push(tag);
        match tag {
            EXT_TAG_CTA => parse_cta(block, &mut info),
            EXT_TAG_DISPLAYID => parse_displayid(block, &mut info),
            _ => {}
        }
    }
    Ok(info)
}

/// 单块 checksum（全部字节之和 mod 256 应为 0；返回值即应写入的校验字节补值）。
pub(crate) fn checksum(block: &[u8]) -> u8 {
    block.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
}

/// 计算块应写入的 checksum 字节（使总和为 0）。
pub(crate) fn checksum_byte(block_without_last: &[u8]) -> u8 {
    let sum: u8 = block_without_last.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    0u8.wrapping_sub(sum)
}

fn parse_base(b: &[u8], info: &mut EdidInfo) {
    // 厂商 ID：big-endian 3×5bit ASCII（A=1）。
    let m = u16::from_be_bytes([b[8], b[9]]);
    let c = |bits: u16| char::from_u32((bits as u32 & 0x1F) + 64).unwrap_or('?');
    info.manufacturer = [c(m >> 10), c(m >> 5), c(m)].iter().collect();
    info.product_code = u16::from_le_bytes([b[10], b[11]]);
    info.serial = u32::from_le_bytes([b[12], b[13], b[14], b[15]]);

    // 4 个 18 字节描述符槽（0x36 起）。
    for off in [0x36usize, 0x48, 0x5A, 0x6C] {
        let d = &b[off..off + 18];
        if d[0] != 0 || d[1] != 0 {
            // DTD
            let t = decode_dtd(d);
            let clock_khz = u16::from_le_bytes([d[0], d[1]]) as u32 * 10;
            let refresh = estimate_refresh(clock_khz, &t);
            info.detailed_timings.push(t);
            info.detailed_refresh_hz.push(refresh);
        } else {
            // 显示器描述符：tag 在 d[3]。
            match d[3] {
                0xFC => {
                    // Monitor Name：d[5..18]，0x0A 结尾。
                    let end = d[5..].iter().position(|&c| c == 0x0A).map(|p| p + 5).unwrap_or(18);
                    let name: String = d[5..end]
                        .iter()
                        .filter(|&&c| (0x20..=0x7E).contains(&c))
                        .map(|&c| c as char)
                        .collect();
                    let name = name.trim().to_string();
                    if !name.is_empty() {
                        info.name = Some(name);
                    }
                }
                0xFD => {
                    // Display Range Limits：d[5..=9] = minV/maxV(Hz) minH/maxH(kHz) maxPclk(×10MHz)。
                    // 0 或 0xFF 表示「未知/见扩展」。
                    if d[6] != 0xFF && d[6] != 0 {
                        info.max_v_rate_hz = Some(d[6] as u32);
                    }
                    if d[8] != 0xFF && d[8] != 0 {
                        info.max_h_freq_khz = Some(d[8] as u32);
                    }
                    if d[9] != 0xFF && d[9] != 0 {
                        info.max_pixel_clock_khz = Some(d[9] as u32 * 10_000);
                    }
                }
                _ => {}
            }
        }
    }
}

/// CTA-861 扩展：DTD 区 + 数据块遍历（HDMI Forum VSDB 的 DSC 线索）。
fn parse_cta(b: &[u8], info: &mut EdidInfo) {
    let dtd_offset = b[2] as usize;
    // 数据块区：4..dtd_offset（dtd_offset==0 表示无 DTD 也无数据块）。
    if dtd_offset >= 4 {
        let mut i = 4usize;
        while i < dtd_offset && i < BLOCK_SIZE {
            let header = b[i];
            let tag = header >> 5;
            let len = (header & 0x1F) as usize;
            let payload_start = i + 1;
            if tag == 0x03 && len >= 3 {
                // Vendor-Specific Data Block：OUI 小端 3 字节。
                let p = &b[payload_start..payload_start + len.min(BLOCK_SIZE - payload_start)];
                let oui = [p[0], p[1], p[2]];
                if oui == [0xD8, 0x5D, 0xC4] {
                    // HDMI Forum VSDB（OUI C4:5D:D8）：DSC 字段在 rev>=1 的扩展区。
                    // byte 13（相对 payload，含 OUI）bit7 = DSC 1.2 支持线索。
                    if p.len() > 13 {
                        info.dsc_hdmi_forum = Some(p[13] & 0x80 != 0);
                    }
                }
            }
            i = payload_start + len;
            if len == 0 && tag == 0 {
                break;
            }
        }
    }
    // DTD 区：dtd_offset..127。
    if dtd_offset >= 4 && dtd_offset < BLOCK_SIZE - 18 {
        let mut off = dtd_offset;
        while off + 18 <= BLOCK_SIZE - 1 {
            let d = &b[off..off + 18];
            if d[0] == 0 && d[1] == 0 {
                break; // 零填充 = DTD 区结束。
            }
            let t = decode_dtd(d);
            let clock_khz = u16::from_le_bytes([d[0], d[1]]) as u32 * 10;
            let refresh = estimate_refresh(clock_khz, &t);
            info.detailed_timings.push(t);
            info.detailed_refresh_hz.push(refresh);
            off += 18;
        }
    }
}

/// DisplayID 扩展：仅探测存在性与版本（timing 注入承载能力）。
fn parse_displayid(b: &[u8], info: &mut EdidInfo) {
    // DisplayID 2.x：b[1] = version（0x20+ 即 2.x）。
    if b.len() > 1 && b[1] >= 0x20 {
        info.displayid_supported = true;
    }
}

/// 18 字节 DTD 解码。
pub(crate) fn decode_dtd(d: &[u8]) -> TimingParams {
    let h_active = d[2] as u32 | (((d[4] >> 4) as u32 & 0xF) << 8);
    let h_blank = d[3] as u32 | (((d[4] & 0xF) as u32) << 8);
    let v_active = d[5] as u32 | (((d[7] >> 4) as u32 & 0xF) << 8);
    let v_blank = d[6] as u32 | (((d[7] & 0xF) as u32) << 8);
    let h_fp = d[8] as u32 | (((d[11] >> 6) as u32 & 0x3) << 8);
    let h_sync = d[9] as u32 | (((d[11] >> 4) as u32 & 0x3) << 8);
    let v_fp = ((d[10] >> 4) as u32 & 0xF) | (((d[11] >> 2) as u32 & 0x3) << 4);
    let v_sync = (d[10] as u32 & 0xF) | ((d[11] as u32 & 0x3) << 4);
    let flags = d[17];
    let h_back = h_blank.saturating_sub(h_fp + h_sync);
    let v_back = v_blank.saturating_sub(v_fp + v_sync);
    TimingParams {
        h_active,
        v_active,
        h_front: h_fp,
        h_sync,
        h_back,
        v_front: v_fp,
        v_sync,
        v_back,
        // 数字分离同步：bit4=1 表示存在 sync 定义；bit1 = h pol, bit0? 布局：
        // bit4: stereo lsb, bit3: sync type(数字分离=1)… 惯例：bit1=H 极性、bit0? 实际：
        // bit3&bit2: 00 模拟复合 / 01 双极模拟 / 10 数字复合 / 11 数字分离；
        // 数字分离时 bit1 = V 极性、bit0?  —— VESA 定义：bit1=HSync pol、bit0 未见；
        // 实际 EDID：d[17] bit1 = Horizontal Sync Polarity（1=+），
        // bit2? 综合多份实现：bits[2:1] = V/H 极性（数字分离时）。采用 bit2=V、bit1=H。
        h_sync_pol: flags & 0x02 != 0,
        v_sync_pol: flags & 0x04 != 0,
        interlaced: flags & 0x80 != 0,
    }
}

/// 由像素时钟与 total 估算刷新率（Hz，四舍五入）。
fn estimate_refresh(clock_khz: u32, t: &TimingParams) -> u32 {
    let total = t.h_total() as u64 * t.v_total() as u64;
    if total == 0 {
        return 0;
    }
    ((clock_khz as u64 * 1000 + total / 2) / total) as u32
}

/// 原生 timing（最高像素时钟的 DTD），供 native-blanking 继承。
pub fn native_timing(info: &EdidInfo) -> Option<&TimingParams> {
    info.detailed_timings.first()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造最小合法 EDID：base block + 1 DTD + name + range limits。
    pub(crate) fn fixture_edid() -> Vec<u8> {
        let mut e = vec![0u8; 128];
        e[0..8].copy_from_slice(&EDID_HEADER);
        // 厂商 "ABC"：A=1,B=2,C=3 → 00001 00010 00011 → 0x04 0x43
        e[8] = 0x04;
        e[9] = 0x43;
        e[10] = 0x34;
        e[11] = 0x12;
        e[126] = 0;
        // DTD @0x36：3840×2160@60，pix clock = 4000×2222×60/1000 = 533,280 kHz = 53,328 (×10kHz)
        // （@240 的 2.13GHz 超 DTD u16 上限，正是 DisplayID 路径存在的意义）
        let pix10: u32 = 53_328;
        let d = &mut e[0x36..0x36 + 18];
        d[0] = (pix10 & 0xFF) as u8;
        d[1] = (pix10 >> 8) as u8;
        let h_act = 3840u32;
        let h_blank = 160u32;
        let v_act = 2160u32;
        let v_blank = 62u32;
        d[2] = h_act as u8;
        d[3] = h_blank as u8;
        d[4] = (((h_act >> 8) as u8) << 4) | ((h_blank >> 8) as u8);
        d[5] = v_act as u8;
        d[6] = v_blank as u8;
        d[7] = (((v_act >> 8) as u8) << 4) | ((v_blank >> 8) as u8);
        // fp/sync：H 48/32，V 3/5
        d[8] = 48;
        d[9] = 32;
        d[10] = (3 << 4) | 5;
        d[11] = 0;
        d[17] = 0x1E; // 数字分离同步，H+/V+
        // Range limits @0x48：tag 0xFD，max pixel clock = 160 ×10MHz = 1.6GHz
        let r = &mut e[0x48..0x48 + 18];
        r[3] = 0xFD;
        r[5] = 24; // min v rate
        r[6] = 240; // max v rate
        r[7] = 30; // min h freq kHz
        r[8] = 250; // max h freq kHz
        r[9] = 160; // max pixel clock ×10MHz
        // Name @0x5A：tag 0xFC "TESTMON"
        let n = &mut e[0x5A..0x5A + 18];
        n[3] = 0xFC;
        n[5..12].copy_from_slice(b"TESTMON");
        n[12] = 0x0A;
        // checksum
        let cks = checksum_byte(&e[..127]);
        e[127] = cks;
        e
    }

    #[test]
    fn parses_base_block() {
        let e = fixture_edid();
        let info = parse(&e).unwrap();
        assert_eq!(info.manufacturer, "ABC");
        assert_eq!(info.product_code, 0x1234);
        assert_eq!(info.name.as_deref(), Some("TESTMON"));
        assert_eq!(info.max_pixel_clock_khz, Some(1_600_000));
        assert_eq!(info.detailed_timings.len(), 1);
        let t = &info.detailed_timings[0];
        assert_eq!(t.h_active, 3840);
        assert_eq!(t.v_active, 2160);
        assert_eq!(t.h_total(), 4000);
        assert_eq!(t.v_total(), 2222);
        assert_eq!(info.detailed_refresh_hz[0], 60);
    }

    #[test]
    fn rejects_bad_checksum() {
        let mut e = fixture_edid();
        e[127] ^= 0xFF;
        assert!(matches!(parse(&e), Err(EdidErr::BadChecksum { block: 0 })));
    }

    #[test]
    fn rejects_short() {
        assert!(matches!(parse(&[0u8; 64]), Err(EdidErr::TooShort(64))));
    }

    #[test]
    fn rejects_bad_header() {
        let mut e = fixture_edid();
        e[0] = 1;
        assert!(matches!(parse(&e), Err(EdidErr::BadHeader)));
    }
}
