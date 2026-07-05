# MainApp — iOS 主 App

- `Views/` — SwiftUI 通用界面与根导航
- `Pairing/` — 配对码输入、密钥协商触发、信任列表
- `DeviceDiscovery/` — Bonjour/mDNS 搜索桌面 Receiver
- `Settings/` — 设置项（Jitter 模式提示、音频参数、关于）
- `BroadcastGuide/` — 引导用户从控制中心开启本 App 的 Broadcast Extension

主 App 完成配对后，将会话信息写入 App Group 供 Extension 读取。
