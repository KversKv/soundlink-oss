# iOS 工程（占位骨架）

Swift + SwiftUI 主 App + ReplayKit Broadcast Upload Extension。

## 目录
- `MainApp/` — 主 App：发现、配对、设置、广播引导
  - `Views/` — SwiftUI 界面
  - `Pairing/` — 配对码输入与信任管理
  - `DeviceDiscovery/` — Bonjour/mDNS 发现桌面端
  - `Settings/` — 设置（Jitter 模式、音频参数展示等）
  - `BroadcastGuide/` — 引导用户从控制中心开启屏幕广播
- `BroadcastExtension/` — 采集与发送（轻量）
- `Shared/` — 主 App 与 Extension 共享（Protocol/Crypto/Models/Logger），通过 App Group

## 合规
仅使用 ReplayKit 官方能力；DRM/受保护内容可能不可采集。Extension 需保持轻量。

## 待办（进入阶段 2 时）
使用 Xcode 初始化工程与 App Group、Broadcast Extension target。
