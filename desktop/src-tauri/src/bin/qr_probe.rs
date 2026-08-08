//! QR-1 M0：平台能力探针 CLI。
//!
//! 输出《平台能力实测报告》到 stdout（JSON），覆盖 display.md §十九-1 的 4 个未知量：
//! - NVAPI 可用性 + DSC 字段
//! - 当前生效的 EDID Override 注册表变体（读得回即生效）
//! - 激活方式（Monitor 重启是否足够——需配合 --test-restart）
//! - 目标模式带宽可行性（--test-mode 1920x1440@480）

use soundlink_lib::features::quick_resolution::platform::windows::{ccd, dsc, edid_reg, gdi, nvapi::NvApi};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let test_mode = parse_flag(&args, "--test-mode"); // 如 1920x1440@480
    let do_restart = args.iter().any(|a| a == "--test-restart");

    println!("{{");
    println!("  \"qr_probe\": \"{}\",", env!("CARGO_PKG_VERSION"));

    // 1) 显示器枚举
    let displays = ccd::enumerate_displays().unwrap_or_default();
    println!("  \"displays\": [");
    for (i, d) in displays.iter().enumerate() {
        let modes = gdi::enum_modes(&d.gdi_name).unwrap_or_default();
        let native_edid = edid_reg::read_native_edid(&d.key.instance_path).unwrap_or_default();
        let edid_info = qr_edid::parse::parse(&native_edid).ok();
        println!("    {{");
        println!("      \"index\": {},", d.index);
        println!("      \"gdi\": \"{}\",", d.gdi_name.replace('\\', "\\\\"));
        println!("      \"name\": \"{}\",", d.friendly_name);
        println!("      \"instance\": \"{}\",", d.key.instance_path.replace('\\', "\\\\"));
        println!("      \"edid_hash\": \"{}\",", d.key.edid_hash);
        println!("      \"is_primary\": {},", d.is_primary);
        if let Some(c) = &d.current {
            println!("      \"current\": \"{}x{} @{}Hz\",", c.width, c.height, c.refresh_hz);
        }
        println!("      \"system_modes\": {},", modes.len());
        if let Some(info) = &edid_info {
            if let Some(m) = info.max_pixel_clock_khz {
                println!("      \"edid_max_pixel_clock_khz\": {},", m);
            }
            println!("      \"edid_displayid\": {},", info.displayid_supported);
            if let Some(dsc) = info.dsc_hdmi_forum {
                println!("      \"edid_dsc_hdmi_forum\": {},", dsc);
            }
        }
        println!("      \"edid_len\": {}", native_edid.len());
        println!("    }}{}", if i + 1 < displays.len() { "," } else { "" });
    }
    println!("  ],");

    // 2) NVAPI
    let nv = NvApi::load();
    println!("  \"nvapi\": {{");
    println!("    \"available\": {},", nv.is_ok());
    if let Ok(api) = &nv {
        let handles = api.display_handles();
        println!("    \"display_handles\": {},", handles.len());
        if let Some(h) = handles.first() {
            if let Ok(link) = api.link_info(*h) {
                println!("    \"lane_count\": {},", link.lane_count);
                println!("    \"rate_gbps\": {},", link.rate_gbps);
                if let Some(b) = link.bpc { println!("    \"bpc\": {},", b); }
                if let Some(cf) = link.color_format { println!("    \"color_format\": \"{}\",", cf); }
                if let Some(s) = link.dsc_supported { println!("    \"dsc_supported\": {},", s); }
                if let Some(e) = link.dsc_enabled { println!("    \"dsc_enabled\": {},", e); }
            }
            if let Ok(t) = api.current_timing(*h) {
                println!("    \"timing\": \"{}x{} total {}x{} pclk {}kHz\",",
                    t.h_active, t.v_active, t.h_total, t.v_total, t.pclk_khz);
            }
        }
    }
    println!("  }},");

    // 3) DSC 判定（第一块屏）
    if let Some(d) = displays.first() {
        let cur = d.current.map(|c| (c.width, c.height, c.refresh_hz));
        let link = nv.as_ref().ok()
            .and_then(|a| a.display_handles().into_iter().next())
            .and_then(|h| nv.as_ref().ok()?.link_info(h).ok());
        let edid_dsc = edid_reg::read_effective_edid(&d.key.instance_path)
            .ok()
            .and_then(|e| dsc::edid_dsc_support(&e));
        let report = dsc::detect(cur, link.as_ref(), edid_dsc, None);
        println!("  \"dsc\": {{");
        println!("    \"state\": \"{:?}\",", report.state);
        if let Some(r) = report.required_gbps { println!("    \"required_gbps\": {:.2},", r); }
        if let Some(a) = report.available_gbps { println!("    \"available_gbps\": {:.2},", a); }
        if let Some(l) = &report.link_label { println!("    \"link\": \"{}\",", l); }
        println!("    \"basis\": {:?}", report.basis);
        println!("  }},");
    }

    // 4) 注册表变体探测（读得回 override 即该变体曾被写入）
    if let Some(d) = displays.first() {
        println!("  \"registry_variants\": {{");
        for v in [qr_ipc::RegVariant::MonitorInstanceOverride, qr_ipc::RegVariant::ClassMonitorOverride, qr_ipc::RegVariant::GraphicsDriversConfiguration] {
            let sub = edid_reg::resolve_variant_subkey(&d.key.instance_path, v);
            let has_override = sub.as_ref().map(|s| {
                edid_reg::read_override(&d.key.instance_path, v).is_ok()
            }).unwrap_or(false);
            println!("    \"{:?}\": {{ \"resolved\": {}, \"has_override\": {} }},",
                v, sub.is_ok(), has_override);
        }
        println!("  }},");
    }

    // 5) 目标模式带宽可行性
    if let Some(mode_str) = test_mode {
        if let Some((w, h, hz)) = parse_mode(&mode_str) {
            let link = nv.as_ref().ok()
                .and_then(|a| a.display_handles().into_iter().next())
                .and_then(|h| nv.as_ref().ok()?.link_info(h).ok());
            if let Some(link) = link {
                let t = qr_edid::timing::generate(qr_edid::timing::TimingStandard::CvtRb2, w, h, hz, None).unwrap();
                let bt = qr_bandwidth::Timing {
                    h_active: t.h_active, v_active: t.v_active,
                    h_total: t.h_total(), v_total: t.v_total(), refresh_hz: hz,
                };
                let spec = if link.rate_gbps >= 10.0 {
                    qr_bandwidth::LinkSpec::dp_uhbr10(link.lane_count)
                } else if link.rate_gbps >= 8.0 {
                    qr_bandwidth::LinkSpec::dp_hbr3(link.lane_count)
                } else {
                    qr_bandwidth::LinkSpec::dp_hbr2(link.lane_count)
                };
                let f = qr_bandwidth::check_feasibility(&bt, link.bpc.unwrap_or(8), qr_bandwidth::ColorFormat::Rgb, &spec, edid_reg::read_effective_edid(&displays[0].key.instance_path).ok().and_then(|e| dsc::edid_dsc_support(&e)).unwrap_or(false), None);
                println!("  \"test_mode\": {{");
                println!("    \"mode\": \"{}x{}@{}Hz\",", w, h, hz);
                println!("    \"pixel_clock_khz\": {},", f.pixel_clock_khz);
                println!("    \"required_uncompressed_gbps\": {:.2},", f.required_uncompressed_gbps);
                println!("    \"available_gbps\": {:.2},", f.available_gbps);
                println!("    \"uncompressed_ok\": {},", f.uncompressed_ok);
                if let Some(d) = f.required_dsc_gbps { println!("    \"required_dsc_gbps\": {:.2},", d); }
                if let Some(d) = f.dsc_ok { println!("    \"dsc_ok\": {},", d); }
                println!("  }},");
            }
        }
    }

    // 6) 设备重启测试（--test-restart，需管理员）
    if do_restart {
        if let Some(d) = displays.first() {
            println!("  \"restart_test\": {{");
            match soundlink_lib::features::quick_resolution::platform::windows::device_restart::restart_device(&d.key.instance_path) {
                Ok(ms) => println!("    \"monitor_restart\": \"ok {}ms\",", ms),
                Err(e) => println!("    \"monitor_restart\": \"fail {}\",", e),
            }
            println!("  }},");
        }
    }

    println!("  \"done\": true");
    println!("}}");
}

fn parse_flag(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1).cloned())
}

fn parse_mode(s: &str) -> Option<(u32, u32, u32)> {
    // "1920x1440@480"
    let (res, hz) = s.split_once('@')?;
    let (w, h) = res.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?, hz.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mode_ok() {
        assert_eq!(parse_mode("1920x1440@480"), Some((1920, 1440, 480)));
        assert_eq!(parse_mode("3840x2160@240"), Some((3840, 2160, 240)));
    }

    #[test]
    fn parse_mode_rejects() {
        assert_eq!(parse_mode("1920x1440"), None);
        assert_eq!(parse_mode("@480"), None);
        assert_eq!(parse_mode("axb@c"), None);
    }
}
