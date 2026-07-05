# 08 · 常见问题与排查

按现象快速定位。技术细节见 [`docs/First/`](../First/SoundLinkStructrue.md)，调试方法见 [06-debug.md](./06-debug.md)。

## 连接 / 发现

**手机搜不到电脑**
- 确认两端同一局域网、同一 Wi-Fi 频段可互通。
- 关闭路由器「AP 隔离 / 访客网络隔离」。
- 电脑防火墙放行 mDNS（UDP 5353）与 SoundLink 的控制 / 音频端口（见 `shared/constants`）。

**配对失败**
- 配对码区分大小写，注意时效（一次性）。
- 时间不同步或密钥协商失败时重新发起配对。

## 音频

**已连接但无声**
- 电脑端确认选对了输出设备且系统音量正常。
- 用抓包确认音频 UDP 包是否到达电脑；到达无声则查桌面 Opus 解码 / Jitter Buffer / 设备输出日志。

**声音卡顿 / 断续**
- 弱网导致丢包；靠近路由器或改用 5GHz。
- 观察 Jitter Buffer 与丢包统计，参考 [`docs/First/06-latency-experience.md`](../First/06-latency-experience.md)。

**延迟明显**
- 属局域网流转特性；电脑端改用有线 / USB / 2.4G 低延迟耳机。
- 实时互动（游戏 / 连麦）不在支持范围。

**采集不到某个 App 的声音**
- iOS：DRM / 受保护内容 / 系统通话音频可能无法采集。
- Android：该应用可能禁止被 `AudioPlaybackCapture` 捕获，属预期限制。
- 详见 [`docs/First/08-platform-notes.md`](../First/08-platform-notes.md)。

## iOS 专项

**广播开不起来 / 列表里没有 SoundLink**
- 确认已安装含 Broadcast Extension 的正式构建。
- 控制中心「屏幕录制」需长按才显示广播目标列表。

**广播中途断开**
- Extension 内存超限；确保未在 Extension 内引入重依赖 / 大缓存。

## Android 专项

**授权弹窗点了允许仍无声**
- 确认前台采集通知存在（Service 存活）。
- 检查 `capture/` 日志中的 MediaProjection 结果码。

## 桌面专项

**Windows 无输出设备 / 崩溃**
- 检查 WebView2 Runtime 是否安装。
- WASAPI 设备插拔后需重选设备。

**macOS 无声**
- 首次运行需授予麦克风 / 音频相关权限（若涉及采集阶段）。

## 提交问题时请附带

- 平台与系统版本、应用构建版本。
- 复现步骤与现象。
- 相关日志（**务必移除配对码 / 密钥等敏感信息**）。
