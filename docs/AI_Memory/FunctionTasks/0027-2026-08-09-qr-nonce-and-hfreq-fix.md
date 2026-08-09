<!-- FT-0027 -->

# QR 预置「BadHandshake: nonce 不匹配」修复 + 2304×1440@165 实机调研（2026-08-09）

> 场景：快速分辨率切换中自定义 `2304×1440@165` 点「立即预置」报
> `辅助进程通信失败：BadHandshake: nonce 不匹配`。要求本机实机预置测试并修复。

## 根因 1（用户报错）：helper 驻留进程持有过期 nonce

- 计划任务 `SoundLink QR Helper` 拉起 helper 后，helper 按设计**空闲驻留 5 分钟**。
- 主进程每次新 helper 会话都会重写 nonce 文件（`%APPDATA%/soundlink/qr_nonce.tmp`，读后即删）。
- 但 `schtasks /Run` 对已运行的任务**不会拉起第二个实例**——驻留的旧 helper 仍用启动时的旧 nonce 校验。
- 结果：第二次及以后的预置/握手必现 `BadHandshake: nonce 不匹配`。

**修复**：[pipe_server.rs](../../../desktop/src-tauri/src/features/quick_resolution/helper_core/pipe_server.rs)
`handle_session` 入口对每个新管道连接重读 nonce 文件刷新期望值（启动 nonce 仅兜底）；
[mod.rs](../../../desktop/src-tauri/src/features/quick_resolution/helper_core/mod.rs) `read_nonce_file` 提为 `pub(crate)`。

**安全性分析**：nonce 文件路径由主进程自己写入且每次会话都重写，管道 ACL 已限定「当前用户 SID + SYSTEM」，
本机无并发的合法第二客户端，重读不引入中间人窗口。

**验证**：`qr_probe --test-helper-session`（新增开关，连续两次握手）。
修复前 `session1 ok / session2 BadHandshake`；替换 helper 后 `session1 ok / session2 ok`，
审计日志出现 `NONCE-REFRESH`。

## 根因 2（实机追加）：timing 生成两类缺陷致驱动裁剪

nonce 修复后 2304×1440@165 仍预置失败（验证环节发现已自动回滚）。逐层定位：

1. **Auto（native-blanking 继承）产物超 DTD 编码上限**：3440×1440 原生 total(3600×1490) 继承给
   2304×1440@165 → pclk≈930MHz 超 DTD 上限 655.35MHz，只能落 DisplayID Type VII，
   而 Windows/NVIDIA 不把 Type VII 枚举为系统模式。修复：[timing.rs](../../../desktop/src-tauri/crates/qr-edid/src/timing.rs)
   `inherit_native` 加 pclk 上限回退 CVT-RB2。
2. **RB2 的 460µs 最小 vblank 使行频超显示器 range limits**：2304×1440@165 RB2 行频 257.2kHz，
   超 CU34G10X 的 maxH=250kHz，驱动静默丢弃。修复：新增 `generate_for_display`，
   行频超上限时压缩 v_back 把行频压回上限（v_back≥8 保底，不足则保持原样由验证兜底）。
   `parse.rs` 同步解析 range limits 的 maxV/maxH。

接线：[provisioner.rs](../../../desktop/src-tauri/src/features/quick_resolution/provisioner.rs)（timing 生成走
`generate_for_display` + 审计 `PROVISION-TIMING`）、[service.rs](../../../desktop/src-tauri/src/features/quick_resolution/service.rs)
（validate 同口径 + `max_h_freq_khz` 透传）、[model.rs](../../../desktop/src-tauri/src/features/quick_resolution/model.rs)/
[ccd.rs](../../../desktop/src-tauri/src/features/quick_resolution/platform/windows/ccd.rs)（`DisplayInfo.max_h_freq_khz`）。

## 实机结论（重要边界，用户需知）

修复后 2304×1440@165 在**本机 AOC CU34G10X 上仍预置失败**——**这不是 bug，是硬件/驱动限制**：

- 注入的 EDID 字节经注册表读回**完全正确**（CTA DTD、2384×1515、595.94MHz、行频 250.0kHz 合规）；
- Monitor 重启（~850ms）与适配器重启（3s 黑屏）两种激活路径都试过，均不枚举 2304×1440；
- 对照实验：2560×1440@60 预置**成功**；1920×1080/1920×1440/2560×1440 @165 等**已在系统列表**
  （EDID 已声明这些分辨率，仅扩刷新率）。

**结论**：该显示器的 NVIDIA 驱动仅支持「EDID 已声明分辨率的刷新率扩展」，拒绝全新分辨率（2304 不在其
EDID 声明集合）。与 CRU 行为一致（CRU 新增分辨率也需重启显卡驱动、且仅限 EDID 已列分辨率）。
2304×1440 无法用 EDID override 注入该显示器。display.md §十七实机矩阵的「NVIDIA 全新分辨率注入」
在此硬件上不成立，M8 NVAPI CustomDisplay（含重启显卡驱动）是后续可选路径。

## 验证结果

- `cargo test --manifest-path crates/qr-edid/Cargo.toml`：**21 全过**（含新增 3 项：
  pclk 超限回退、行频压缩、未超限不变）。
- `cargo clippy --features tauri_app`：无 error/warning。
- 实机：nonce 双会话握手 ok；2304 注入字节正确但驱动不枚举（硬件限制）；2560@60 预置成功；现场已还原干净。

## 交付/产物

- dist 里 `qr_helper.exe` 已替换为修复版（驻留进程已由 `schtasks /End` 终止，下次预置自动用新二进制）。
- 临时探针 `qr_probe_runner.ps1` / `qr_probe.exe` / 各 `qr_*.json` 已删除。
- CHANGELOG `[未发布]` 修复小节回填两条。

## 已知边界 / 后续

- NVAPI `GetDisplayPortInfo` 结构版本不兼容（`NVAPI_INCOMPATIBLE_STRUCT_VERSION`，code=-9），
  DSC 链路信息拿不到（既有问题，FT-0022 已记 M8 保守返回）。
- NVAPI `GetTiming` 句柄指针为 None（"未检测到 NVIDIA 驱动接口"）。
- M8 NVAPI CustomDisplay（Try/Save/Revert）与「重启显卡驱动激活」未实现——是突破「仅 EDID 已声明分辨率」
  限制的可选方向。

## 建议版本级别

**PATCH**（缺陷修复，无新能力、无破坏性变更）。

## 关键文件索引

- 修复：`helper_core/pipe_server.rs`（nonce 重读）、`helper_core/mod.rs`（pub(crate)）、
  `crates/qr-edid/src/timing.rs`（inherit 回退 + generate_for_display）、`crates/qr-edid/src/parse.rs`（range limits）、
  `provisioner.rs` / `service.rs` / `model.rs` / `ccd.rs`（接线）、`bin/qr_probe.rs`（诊断开关）。
- 关联：[FT-0022](./0022-2026-08-08-quick-resolution-qr1.md)（QR-1 全量）、
  [FT-0026](./0026-2026-08-08-qr-helper-install-error-q.md)（helper 安装）。
