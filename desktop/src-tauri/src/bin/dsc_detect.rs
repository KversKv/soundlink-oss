//! 本机 DSC 判定（带宽推算主判据，display.md §6.1 第①路）。
//! 不依赖 NVAPI 链路字段（GetDisplayPortInfo 参数存疑），用 EDID 解析 + 当前模式 + 带宽公式。

fn main() {
    println!("=== 本机 DSC 判定（带宽推算法） ===\n");

    // 枚举显示器
    let displays = match soundlink_lib::features::quick_resolution::platform::windows::ccd::enumerate_displays() {
        Ok(d) => d,
        Err(e) => { println!("枚举失败：{}", e); return; }
    };

    for d in &displays {
        println!("显示器 {}: {} ({})", d.index, d.friendly_name, d.gdi_name);
        let cur = match &d.current {
            Some(c) => c,
            None => { println!("  无当前模式，跳过\n"); continue; }
        };
        println!("  当前模式：{}×{} @{}Hz", cur.width, cur.height, cur.refresh_hz);

        // EDID
        let edid = soundlink_lib::features::quick_resolution::platform::windows::edid_reg::read_effective_edid(&d.key.instance_path).unwrap_or_default();
        let edid_info = qr_edid::parse::parse(&edid).ok();
        let mut dsc_supported = false;
        let mut max_tmds_or_dp = String::new();
        if let Some(info) = &edid_info {
            if let Some(m) = info.max_pixel_clock_khz {
                max_tmds_or_dp = format!("EDID 像素时钟上限 {:.0} MHz", m as f32 / 1000.0);
            }
            if info.dsc_hdmi_forum == Some(true) { dsc_supported = true; }
            if info.displayid_supported { /* DisplayID 存在 */ }
            println!("  {}", max_tmds_or_dp);
            if let Some(dsc) = info.dsc_hdmi_forum {
                println!("  HDMI Forum VSDB DSC 字段：{}", dsc);
            }
        }

        // 带宽推算（主判据）：当前模式未压缩需求 vs 常见链路净带宽
        let t = qr_bandwidth::Timing {
            h_active: cur.width, v_active: cur.height,
            h_total: (cur.width as f32 * 1.05) as u32,
            v_total: (cur.height as f32 * 1.02) as u32,
            refresh_hz: cur.refresh_hz,
        };
        let need_8bpc = t.required_gbps(8, qr_bandwidth::ColorFormat::Rgb);
        let need_10bpc = t.required_gbps(10, qr_bandwidth::ColorFormat::Rgb);
        println!("  未压缩需求：{:.1} Gbps (8bpc) / {:.1} Gbps (10bpc)", need_8bpc, need_10bpc);

        // 常见链路净带宽（8b/10b 0.8 效率；UHBR 128b/132b 0.9697）
        let links = [
            ("DP1.2 HBR2 ×4", 4.0 * 5.4 * 0.8),
            ("DP1.4 HBR3 ×4", 4.0 * 8.1 * 0.8),
            ("DP2.1 UHBR10 ×4", 4.0 * 10.0 * 0.9697),
            ("DP2.1 UHBR13.5 ×4", 4.0 * 13.5 * 0.9697),
            ("HDMI 2.1 FRL6 ×4", 4.0 * 12.0 * 0.8889),
        ];
        println!("  判定：");
        let mut verdict = String::new();
        for (name, avail) in links {
            let fits = need_8bpc <= avail * 0.98;
            println!("    {} 净 {:.1} Gbps → {}", name, avail, if fits { "可未压缩" } else { "需 DSC" });
            if !fits && verdict.is_empty() {
                verdict = format!("若以 {} 运行当前模式，则 DSC 必然启用", name);
            }
        }
        // 综合结论
        let hbr3_avail = 4.0 * 8.1 * 0.8 * 0.98;
        let uhbr10_avail = 4.0 * 10.0 * 0.9697 * 0.98;
        if need_8bpc <= hbr3_avail {
            verdict = format!("当前模式（{:.1} Gbps）在 DP1.4 HBR3×4 净带宽内，DSC 很可能【未启用】", need_8bpc);
        } else if need_8bpc <= uhbr10_avail {
            verdict = format!("当前模式（{:.1} Gbps）超出 HBR3 但在 UHBR10 内：DP2.1 下 DSC 可能未启用，DP1.4 下必然启用", need_8bpc);
        } else {
            verdict = format!("当前模式（{:.1} Gbps）超出 UHBR10 净带宽，DSC 必然【启用】", need_8bpc);
        }
        println!("  >>> {}", verdict);
        if dsc_supported {
            println!("  （EDID 声明显示器支持 DSC）");
        }
        println!();
    }

    println!("=== 说明 ===");
    println!("本判定为带宽推算（第①路主判据）。NVAPI GetDisplayPortInfo 的精确结构体");
    println!("布局/参数在本机驱动上未探测成功（全部返回 NVAPI_INVALID_ARGUMENT），");
    println!("故第②路（NVAPI 直读 DSC 字段）不可用——这正是 M0 需要实测的未知量。");
}
