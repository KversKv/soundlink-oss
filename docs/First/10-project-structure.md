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
│   ├── ios/                  # iOS 工程
│   │   ├── MainApp/
│   │   │   ├── Views/
│   │   │   ├── Pairing/
│   │   │   ├── DeviceDiscovery/
│   │   │   ├── Settings/
│   │   │   └── BroadcastGuide/
│   │   ├── BroadcastExtension/
│   │   │   ├── SampleHandler.swift
│   │   │   ├── AudioProcessor.swift
│   │   │   ├── OpusEncoderWrapper.swift
│   │   │   ├── UdpAudioSender.swift
│   │   │   └── PairingStateReader.swift
│   │   └── Shared/
│   │       ├── Protocol/
│   │       ├── Crypto/
│   │       ├── Models/
│   │       └── Logger/
│   │
│   └── android/              # Android 工程
│       └── app/src/main/java/com/soundlink/
│           ├── ui/           # Compose 界面
│           ├── pairing/
│           ├── discovery/
│           ├── capture/      # MediaProjection + AudioPlaybackCapture
│           ├── codec/        # Opus 封装
│           ├── network/      # UDP/控制
│           ├── crypto/
│           └── model/
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
| `mobile/ios` | iOS 主 App + Broadcast Extension |
| `mobile/android` | Android 主 App + 采集 Service |
| `desktop/src-tauri` | Rust 核心（网络/音频/配对/设备） |
| `desktop/ui` | Tauri 前端界面 |
| `shared` | 各端一致的协议与常量定义 |
| `docs` | 设计文档 |

## 说明

- 本阶段仓库为**骨架 + 占位说明**：各关键文件/目录含 README 或占位文件，标注职责与待实现内容，不含可运行实现。
- 具体脚手架初始化（`tauri init`、Xcode 工程、Gradle 工程）在进入对应阶段时执行，见 [09-roadmap](./09-roadmap.md)。