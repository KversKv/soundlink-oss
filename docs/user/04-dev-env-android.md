# 04 · 开发环境搭建 · Android 发送端

Android 端为音频发送端，基于 **Kotlin + Jetpack Compose + MediaProjection + AudioPlaybackCapture（API 29+）**。仅使用官方采集能力，**不依赖 root / 私有权限**。

可在 Windows / macOS / Linux 上开发。先完成 [01-dev-env-common.md](./01-dev-env-common.md) 的通用前置。

## 1. 环境要求

| 依赖 | 说明 |
|---|---|
| JDK | 17（Android Gradle Plugin 近版本要求） |
| Android Studio | 最新稳定版（含 SDK Manager / AVD） |
| Android SDK | 编译 SDK + Platform Tools；最低 API 29（AudioPlaybackCapture 要求） |
| 真机 Android 设备 | 推荐真机（API 29+）；AudioPlaybackCapture 在模拟器上受限 |

Android Studio 会引导安装 SDK、构建工具与平台工具，无需手动配 Gradle（使用 Gradle Wrapper）。

## 2. 工程结构

Android 工程位于 [`mobile/android`](../../mobile/android)，包 `com.soundlink`：

- `ui/` — Compose 界面
- `pairing/` — 配对
- `discovery/` — NSD / mDNS 设备发现
- `capture/` — MediaProjection + AudioPlaybackCapture 前台 Service
- `codec/` — Opus 封装（JNI）
- `network/` — UDP / 控制
- `crypto/`、`model/` — 加密与模型

## 3. 关键配置

- **前台 Service**：采集载体为 `mediaProjection` 类型前台 Service，需在通知栏展示采集状态（合规要求）。
- **权限**：`FOREGROUND_SERVICE` / `FOREGROUND_SERVICE_MEDIA_PROJECTION`、录音相关权限，以及 MediaProjection 运行时授权弹窗。
- **NSD**：设备发现使用 Network Service Discovery。
- **密钥存储**：Android Keystore / EncryptedSharedPreferences。
- **libopus**：JNI 或成熟 Opus 封装（脚手架阶段确定）。

> 采集范围：`AudioPlaybackCapture` 仅能采集允许被捕获的应用音频，部分应用 / 受保护内容不可采，见 [`docs/First/08-platform-notes.md`](../First/08-platform-notes.md)。

## 4. 打开与运行

> 脚手架就绪后：

1. Android Studio 打开 `mobile/android` 目录。
2. 等待 Gradle Sync 完成、SDK 下载。
3. 连接真机（开启开发者选项 + USB 调试）或启动 AVD。
4. 选择 `app` 配置，Run。

命令行构建：

```bash
cd mobile/android
./gradlew assembleDebug      # Windows: .\gradlew.bat assembleDebug
```

## 5. Lint

```bash
cd mobile/android
./gradlew lint ktlintCheck   # 具体任务以工程配置为准
```

编译打包见 [05-build.md](./05-build.md)，调试见 [06-debug.md](./06-debug.md)。
