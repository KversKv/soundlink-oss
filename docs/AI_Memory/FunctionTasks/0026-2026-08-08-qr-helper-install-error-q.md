<!-- FT-0026 -->
# QR 安装辅助组件报「辅助进程通信失败：q」修复实录（2026-08-08）

> 场景：快速分辨率切换区点击「安装辅助组件（一次 UAC）」，提示「辅助进程通信失败：q」。

## 根因（双重缺陷）

1. **前端错误解析截断字符串**：`QrError::HelperIpc(String)` 等 newtype 变体经
   serde `tag="code", content="detail"` 序列化后，`detail` 是**纯字符串**；
   `parseQrError` 却统一 `as Record<string, unknown>` 并取 `d[0]`，把真实消息
   「qr_helper.exe 不存在（需随包发布）」截成首字符 `"q"`。
2. **安装包未携带 helper**：`tauri.conf.json` 未配置任何资源，NSIS/MSI 安装目录里
   没有 `qr_helper.exe`，`install_helper()` 的 `helper.exists()` 守卫命中，返回上述
   HelperIpc 错误——只是被缺陷 1 遮蔽成 ":q"。

## 实现清单

| 文件 | 改动 |
|---|---|
| `desktop/ui/src/features/quickResolution/types.ts` | `HelperIpc`/`BadRequest`/`Edid`/`Io` 四分支改取 `typeof p.detail === "string" ? p.detail : ""` |
| `desktop/src-tauri/tauri.conf.json` | bundle 新增 `"resources": ["target/release/qr_helper.exe"]`（相对 src-tauri；`tauri build` 先 cargo build 全部 bin 再打包，文件必然存在；Windows 下 resource_dir = 安装目录，helper 落在主程序旁） |
| `desktop/src-tauri/src/main.rs` | 主程序被复制/重命名为 `qr_helper.exe` 时，在最顶端（先于 Tauri/单实例初始化）分发到 `helper_core::run`，进入提权辅助进程模式 |
| `desktop/src-tauri/src/.../windows/helper_client.rs` | `install_helper()` 在 `qr_helper.exe` 缺失时把主程序复制为同目录 `qr_helper.exe` 自举（便携单文件形态），复制失败给出「目录需可写」明确提示 |
| `desktop/src-tauri/src/.../quick_resolution/service.rs` | 预置守卫改实时探测 `helper_installed()`（不再信内存标志）；新增 `mark_helper_installed()` |
| `desktop/src-tauri/src/.../quick_resolution/commands.rs` | `qr_install_helper` 成功后调 `mark_helper_installed()` 持久化标志 |
| `desktop/ui/.../quickResolution/api.ts` / `QuickResolutionSection.tsx` | 新增 `helperStatus()`；加载与安装成功均以实时 `qr_helper_status` 覆盖 `helperInstalled` |
| `CHANGELOG.md` | `[未发布]` 修复小节回填 |

## 根因 3（实机实测追加）：helper_installed 标志失真

实测环境：计划任务 `SoundLink QR Helper` 已正确注册（`dist\pro\qr_helper.exe --serve`、
RunLevel=Highest、exe 存在、版本一致），但点「立即预置」仍报 `HelperNotInstalled`。

根因：`provision()` 前置守卫读**内存 `s.helper_installed`**（来自落盘设置）；而前端点
「安装辅助组件」只在 React `setSettings` 改了内存 state、**从未调 `qr_set_settings` 持久化**，
重启 App 后标志回落 false。后端虽有 `qr_helper_status` 实时探测命令，却无人调用——守卫信了
陈旧标志，把已装好的任务挡在门外。

## 关键设计决策

- 不用 externalBin/sidecar：sidecar 打包后保留 target-triple 后缀名，与 `install_helper`
  固定查找 `qr_helper.exe` 不匹配；resources 数组形式原名落安装目录，零运行时改动。
- **便携形态自举**（用户追问后追加）：主程序与 helper 同一 crate/版本，文件名切换角色，
  天然满足版本绑定；`verify_client` 仅要求客户端与 helper 同目录，任意位置便携目录可通过。
  主程序按 `file_stem() == "qr_helper"` 判定——用户自行重命名 soundlink.exe 不影响，
  且任何被命名为 qr_helper.exe 的副本都是 SoundLink 自身二进制（helper 功能仅经
  安装版/便携版主程序触发）。
- 主程序 GUI 为 release `windows_subsystem = "windows"`，helper 模式的 eprintln 诊断
  在 GUI 子系统下不可见——与独立 qr_helper.exe 行为一致，维持现状。
- macOS 打包本就不支持（`helper_core` 仅 `cfg(all(windows, feature = "tauri_app"))`，
  qr_helper bin 在非 Windows 不可编译），Windows-only 资源路径不引入跨平台回归。

## 验证结果

- `npx tsc -b`（desktop/ui）：通过。
- `cargo check --no-default-features --features tauri_app`：通过。
- `cargo clippy --no-default-features --features tauri_app --lib --bins`：0 error（仅 nvapi_probe 既有 warning）。
- NSIS 重打包后安装目录应含 `qr_helper.exe`（待用户实测安装链路 + 一次 UAC）。

## 用户需自行完成部分

1. 重新 `tauri build --features tauri_app` 后实测两条链路：
   - 安装版：点「安装辅助组件」→ 一次 UAC → 计划任务注册成功。
   - 便携版：把便携 exe 放到可写目录，点「安装辅助组件」→ 自动生成 qr_helper.exe → 一次 UAC。
2. 关于「能否每次操作直接弹 UAC（不装计划任务）」：可行但非推荐——每次切换/预置
   都弹 UAC 且以完整管理员身份执行；计划任务 + 管道 + nonce + 命令白名单的方案把提权
   面缩到最小（display.md §4.2）。如确需该模式可后续加 sidecar fallback。

## 追加功能（用户要求）：管理员直写路径

需求：主程序检测到自身已是管理员时，跳过计划任务，直接在本进程内做 EDID 注入。

| 文件 | 改动 |
|---|---|
| `platform/windows/direct_admin.rs`（新增） | `is_elevated()`（OpenProcessToken+TokenElevation）+ `write_override`/`remove_override`/`restart_monitor`/`restart_adapter`（复用 edid_reg/device_restart） |
| `platform/windows/ccd.rs` | 新增 `first_active_adapter()`（pub(crate)，取第一条活动路径 adapter LUID+source id，供适配器重启） |
| `platform/windows/mod.rs` | 注册 `direct_admin` 模块 |
| `provisioner.rs` | 新增 `PrivilegedOps` 双路径执行器（Direct 直写 / Helper 转发，按 `is_elevated()` 自动选）；预置第 4-7 段改用执行器 |
| `service.rs` | 预置守卫放宽为「is_elevated() 或 helper_installed()」任一放行 |

**关键决策（用户拍板）**：看门狗保留走 helper——直写仅替代「写/删 override + 设备重启」，
武装/解除看门狗仍建立 helper 会话。原因：看门狗是独立进程守护，主进程崩溃时由它还原 EDID
防黑屏，主进程无法自我守护。故直写路径下计划任务仍需存在（仅用于看门狗，不再用于写入转发）。

验证：`cargo check` 0 error；`cargo test ... quick_resolution` 42 全绿；clippy 0 error。

## 建议版本级别

PATCH（缺陷修复，无协议/格式变更；安装包内容变化属构建产物修正）。

## 关联文档

- [FT-0022](./0022-2026-08-08-quick-resolution-qr1.md)（QR-1 落地）
- `docs/NewFunctions/display/display.md` §4（helper 架构）
