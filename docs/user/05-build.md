# 05 · 编译 / 打包方式

各端的开发运行、编译与产物打包命令。环境搭建见对应平台文档（[桌面](./02-dev-env-desktop.md) / [iOS](./03-dev-env-ios.md) / [Android](./04-dev-env-android.md)）。

> 仓库处于骨架阶段，以下命令为脚手架就绪后的目标工作流。

## 1. 桌面端（Tauri 2 + Rust）

### 开发运行（热重载）

```bash
cd desktop/src-tauri
cargo tauri dev
```

### 仅编译 Rust 核心

```bash
cd desktop/src-tauri
cargo build            # debug
cargo build --release  # release
```

### 打包安装包

```bash
cd desktop/src-tauri
cargo tauri build
```

产物位置（`target/release/bundle/` 下）按平台不同：

| 平台 | 产物 |
|---|---|
| Windows | `.msi` / `.exe`（NSIS） |
| macOS | `.app` / `.dmg`（需签名 / 公证以分发） |
| Linux | `.deb` / `.AppImage`（后续阶段） |

## 2. iOS

iOS 主 App 由 **Flutter** 构建，原生 Broadcast Extension 随之一起打包，需 macOS + Xcode。

### Flutter 构建

```bash
cd mobile/flutter_app
flutter build ios --release        # 产出 Runner.app（含 Extension）
```

### Xcode 内构建 / 调试

```bash
open mobile/flutter_app/ios/Runner.xcworkspace
```

- 选择 `Runner` scheme + 真机 → **Product → Build / Run**。
- Broadcast Extension（原生 Swift）随主 App 一起打包（同一 App Group）。

### 归档 / 分发

```bash
cd mobile/flutter_app
flutter build ipa --release        # 产出 .ipa 供上传
```

- 或在 Xcode **Product → Archive** 生成 `.xcarchive`，经 Organizer 导出 IPA / 上传 App Store Connect。
- 需有效的开发者签名与描述文件。

## 3. Android

Android 主 App 由 **Flutter** 构建（底层走 Gradle Wrapper，无需本地装 Gradle），原生采集 Service 一并打包。

```bash
cd mobile/flutter_app

# Debug APK
flutter build apk --debug

# Release APK
flutter build apk --release

# Play 上架用 AAB
flutter build appbundle --release
```

产物：

| 类型 | 路径（相对 `mobile/flutter_app`） |
|---|---|
| APK | `build/app/outputs/flutter-apk/` |
| AAB | `build/app/outputs/bundle/release/` |

> Release 构建需配置签名（`keystore`），请勿将 keystore / 密码提交到仓库。

## 4. 共享层

`shared/` 为协议与常量定义，无独立可执行产物；改动后需保证各端一致并同步 [`docs/First/04-protocol.md`](../First/04-protocol.md)。
