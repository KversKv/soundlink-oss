# 10 · 项目结构（Project Structure）

顶层按**移动端 / 桌面端 / 共享 / 文档**划分：

```text
SoundLink/
├── AGENTS.md                 # TRAE/AI 协作说明（仓库根）
├── .trae/rules/project-rules.md  # TRAE 项目级工程规则
├── README.md                 # 仓库入口说明
├── docs/                     # 全部设计文档
│   └── First/                # 第一阶段规划文档（本目录）
│
├── mobile/                   # 移动端（iOS + Android）
│   ├── flutter_app/           # 当前移动端主工程（Flutter UI + iOS/Android 宿主）
│   │   ├── lib/               # 跨端 UI、发现、配对、设置、广播引导
│   │   ├── android/           # Flutter Android 宿主 + Kotlin 采集 Service + JNI/CMake/libopus
│   │   ├── ios/               # Flutter iOS Runner + BroadcastExtension target 配置
│   │   └── test/              # Flutter widget/protocol 测试
│   │
│   ├── ios/                  # iOS 原生采集源码（由 flutter_app/ios Xcode target 引用）
│   │   └── BroadcastExtension/
│   │       ├── SampleHandler.swift
│   │       ├── AudioProcessor.swift
│   │       ├── OpusEncoderWrapper.swift
│   │       ├── UdpAudioSender.swift
│   │       └── PairingStateReader.swift
│   │
│   └── android/              # 早期 Android 原生结构参考；当前构建入口以 flutter_app/android 为准
│
├── desktop/                  # 桌面端（Tauri 2 + Rust）
│   ├── src-tauri/
│   │   └── src/
│   │       ├── main.rs
│   │       ├── commands/     # 暴露给前端的命令
│   │       ├── audio/
│   │       │   ├── output/
│   │       │   │   ├── windows_wasapi.rs
│   │       │   │   ├── macos_coreaudio.rs
│   │       │   │   └── linux_pipewire.rs
│   │       │   ├── jitter_buffer.rs
│   │       │   ├── opus_decoder.rs
│   │       │   └── resampler.rs
│   │       ├── network/
│   │       │   ├── discovery.rs
│   │       │   ├── udp_receiver.rs
│   │       │   ├── control_server.rs
│   │       │   └── packet.rs
│   │       ├── pairing/
│   │       │   ├── pairing_code.rs
│   │       │   ├── key_exchange.rs
│   │       │   └── trust_store.rs
│   │       ├── device/
│   │       │   ├── audio_device.rs
│   │       │   └── device_identity.rs
│   │       ├── config/
│   │       └── logging/
│   └── ui/
│       └── src/
│           ├── pages/
│           ├── components/
│           └── stores/
│
└── shared/                   # 跨端共享约定
    ├── protocol/             # 协议定义（消息/包结构/错误码）
    └── constants/            # 服务类型、端口、音频参数
```

## 目录职责

| 目录 | 职责 |
|---|---|
| `mobile/flutter_app` | 当前移动端主工程：Flutter UI + iOS/Android 宿主 |
| `mobile/flutter_app/android` | Android 真机入口：MediaProjection Service + JNI/CMake/libopus |
| `mobile/flutter_app/ios` | iOS 真机入口：Runner + BroadcastExtension target/App Group 配置 |
| `mobile/ios/BroadcastExtension` | iOS Broadcast Extension Swift 源码 |
| `mobile/android` | 早期 Android 原生结构参考；不作为当前默认构建入口 |
| `desktop/src-tauri` | Rust 核心（网络/音频/配对/设备） |
| `desktop/ui` | Tauri 前端界面 |
| `shared` | 各端一致的协议与常量定义 |
| `docs` | 设计文档 |

## 说明

- 当前仓库已从纯骨架进入可构建实现阶段：桌面 Rust/Tauri、Flutter 主 App、Android Gradle/CMake/libopus 链路已具备本地构建验证。
- 移动端当前唯一默认 Flutter 工程为 `mobile/flutter_app`；历史重复工程已清理，避免后续维护时误改。
- iOS BroadcastExtension 源码保留在 `mobile/ios/BroadcastExtension`，由 `mobile/flutter_app/ios/Runner.xcodeproj` 的 BroadcastExtension target 引用；真机验收仍需 macOS/Xcode、签名/App Group provisioning 与 libopus xcframework。
- Android 真机入口为 `mobile/flutter_app/android`；`mobile/android` 仅保留早期原生结构参考，后续若继续保留需避免与 Flutter 宿主重复实现。