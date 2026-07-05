# Android 工程（占位骨架）

Kotlin + Jetpack Compose 主 App + MediaProjection 前台采集 Service。

包根：`com.soundlink`

## 模块
- `ui/` — Compose 界面（发现/配对/状态/设置）
- `pairing/` — 配对码、密钥协商、信任存储（Keystore/EncryptedPrefs）
- `discovery/` — NSD/mDNS 发现桌面端
- `capture/` — MediaProjection + AudioPlaybackCapture + AudioRecord 采集
- `codec/` — Opus 编码封装（JNI/成熟库）
- `network/` — UDP 音频发送 + 控制通道（WebSocket/TCP）
- `crypto/` — ChaCha20-Poly1305 / X25519
- `model/` — 设备/配对/会话模型

## 合规
需前台 Service（`mediaProjection` 类型）+ 用户授权 + 通知栏状态；部分应用/受保护内容不可采（API 29+）。

## 待办（进入阶段 2 时）
使用 Android Studio / Gradle 初始化工程，声明前台服务与权限。
