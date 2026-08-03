# mobile/ios — ReplayKit Broadcast Extension 源码

> Xcode 工程入口在 [`mobile/flutter_app/ios`](../flutter_app/ios)（`Runner.xcworkspace`）。本目录存放 Broadcast Upload Extension 的 Swift 实现源码与主 App 分层参考。

## 已实现（BroadcastExtension）

| 文件 | 职责 |
|---|---|
| `BroadcastExtension/SampleHandler.swift` | ReplayKit 回调入口，接收 `audioApp` sample buffer |
| `BroadcastExtension/AudioProcessor.swift` | PCM 归一化到 48 kHz / Stereo / 10 ms 帧 |
| `BroadcastExtension/OpusEncoderWrapper.swift` | libopus 编码（依赖 `Opus.xcframework`） |
| `BroadcastExtension/UdpAudioSender.swift` | 加密 + UDP 发送到桌面接收端 |
| `BroadcastExtension/PairingStateReader.swift` | 通过 App Group 读取主 App 写入的配对/会话状态 |

Extension 侧 plist / entitlements 在 `mobile/flutter_app/ios/BroadcastExtension/`。

## 主 App 分层参考（`MainApp/`、`Shared/`）

主 App 的发现、配对、设置、广播引导已由 Flutter 实现（`mobile/flutter_app/lib/src/pages`、`.../services`）。`MainApp/` 与 `Shared/` 下的 README 保留为设计分层说明，不含可编译源码。

## 合规要点

仅使用 ReplayKit 官方能力；DRM / 受保护内容可能不可采集；Extension 内存上限严格，必须保持轻量（禁止重依赖与大缓冲）。详见 `docs/First/08-platform-notes.md`。

## 构建

```bash
cd mobile/flutter_app/ios
./scripts/build_opus_xcframework.sh   # 生成 Frameworks/Opus.xcframework
open Runner.xcworkspace               # 配置签名与 App Group 后运行
```

需 macOS + Xcode；当前状态 🟡 **待真机验收**。环境与步骤见 `docs/user/03-dev-env-ios.md`。
