# 04 · 开发环境搭建 · Android 发送端

Android 端为音频发送端，采用「**Flutter 主 App + 原生采集 Service**」分层架构：主 App UI 用 **Flutter（Dart）** 与 iOS 共用一套；系统音频采集用 **原生 Kotlin + MediaProjection + AudioPlaybackCapture（API 29+）**。仅使用官方采集能力，**不依赖 root / 私有权限**。架构决策见 [`docs/First/07-tech-stack.md`](../First/07-tech-stack.md) §6、[`docs/First/08-platform-notes.md`](../First/08-platform-notes.md) §1b。

可在 Windows / macOS / Linux 上开发。先完成 [01-dev-env-common.md](./01-dev-env-common.md) 的通用前置。

## 1. 环境要求

| 依赖 | 说明 |
|---|---|
| Flutter SDK | 稳定版（含 Dart）；`flutter doctor` 全绿 |
| JDK | 17（Android Gradle Plugin 近版本要求） |
| Android Studio | 最新稳定版（含 SDK Manager / AVD、Flutter/Dart 插件） |
| Android SDK | 编译 SDK + Platform Tools；最低 API 29（AudioPlaybackCapture 要求） |
| 真机 Android 设备 | 推荐真机（API 29+）；AudioPlaybackCapture 在模拟器上受限 |

Android Studio 会引导安装 SDK、构建工具与平台工具；Flutter 通过 Gradle Wrapper 构建 Android，无需手动装 Gradle。校验：

```bash
flutter doctor          # 按提示补齐 Android 工具链、接受 SDK 许可
```

## 2. 工程结构

移动端 Flutter 主 App 位于 [`mobile/flutter_app`](../../mobile/flutter_app)（iOS/Android 共用）；Android 原生宿主与采集组件位于其 `android/` 目录，包 `com.soundlink`：

- `mobile/flutter_app/lib/` — Flutter 主 App（配对、发现、设置、授权引导 UI，Dart）
- `mobile/flutter_app/android/app/` — Android 原生宿主（承载 Flutter 引擎 + 与采集 Service 桥接）
- 原生采集组件（**Kotlin**，通过 Platform Channel 与 Flutter 主 App 通信）：
  - `capture/` — MediaProjection + AudioPlaybackCapture 前台 Service（采集 + Opus 编码 + UDP 发送）
  - `codec/` — Opus 封装（JNI）
  - `network/` — UDP / 控制
  - `crypto/`、`model/` — 加密与模型

> **Flutter 只在主 App 进程**；前台采集 Service 是独立组件，保持纯原生轻量。

## 3. 关键配置

- **前台 Service**：采集载体为 `mediaProjection` 类型前台 Service，需在通知栏展示采集状态（合规要求），原生 Kotlin 实现。
- **权限**：`FOREGROUND_SERVICE` / `FOREGROUND_SERVICE_MEDIA_PROJECTION`、录音相关权限，以及 MediaProjection 运行时授权弹窗。
- **主 App ↔ Service 通信**：Flutter 主 App 经 Platform Channel 调起/配置原生采集 Service；音频包在 Service 内直接编码发送，不回传主 App。
- **NSD**：设备发现使用 Network Service Discovery。
- **密钥存储**：Android Keystore / EncryptedSharedPreferences。
- **libopus**：JNI 或成熟 Opus 封装（脚手架阶段确定）。

> 采集范围：`AudioPlaybackCapture` 仅能采集允许被捕获的应用音频，部分应用 / 受保护内容不可采，见 [`docs/First/08-platform-notes.md`](../First/08-platform-notes.md)。

## 4. 打开与运行

> 脚手架就绪后：

主 App 用 Flutter 命令运行（真机）：

```bash
cd mobile/flutter_app
flutter pub get
flutter run -d <android-device-id>     # flutter devices 查看设备 id
flutter run -d 41091JEKB06514
```

需要调试/配置原生采集组件时，用 Android Studio 打开 `mobile/flutter_app/android`（或用根目录 Flutter 工程，选 Android 模块）：

1. 连接真机（开启开发者选项 + USB 调试）或启动 AVD。
2. 等待 Gradle Sync 完成、SDK 下载。
3. Run。

## 5. Lint

```bash
cd mobile/flutter_app
flutter analyze           # Dart / Flutter 侧
```

原生 Kotlin 侧 lint 以工程配置为准（`./gradlew lint` 等，位于 `android/`）。

编译打包见 [05-build.md](./05-build.md)，调试见 [06-debug.md](./06-debug.md)。
