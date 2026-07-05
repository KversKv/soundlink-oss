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

iOS 通过 **Xcode** 构建，需 macOS。

### Xcode 内构建

- 选择 `MainApp` scheme + 真机 → **Product → Build / Run**。
- Broadcast Extension 随主 App 一起打包（同一 App Group）。

### 命令行构建（CI）

```bash
xcodebuild -workspace mobile/ios/SoundLink.xcworkspace \
  -scheme MainApp -configuration Release \
  -destination 'generic/platform=iOS' build
```

### 归档 / 分发

- **Product → Archive** 生成 `.xcarchive`，通过 Organizer 导出 IPA 或上传 App Store Connect。
- 需有效的开发者签名与描述文件。

## 3. Android

Android 使用 **Gradle Wrapper**（无需本地装 Gradle）。

```bash
cd mobile/android

# Debug APK
./gradlew assembleDebug        # Windows: .\gradlew.bat assembleDebug

# Release APK
./gradlew assembleRelease

# Play 上架用 AAB
./gradlew bundleRelease
```

产物：

| 类型 | 路径 |
|---|---|
| Debug APK | `app/build/outputs/apk/debug/` |
| Release APK | `app/build/outputs/apk/release/` |
| AAB | `app/build/outputs/bundle/release/` |

> Release 构建需配置签名（`keystore`），请勿将 keystore / 密码提交到仓库。

## 4. 共享层

`shared/` 为协议与常量定义，无独立可执行产物；改动后需保证各端一致并同步 [`docs/First/04-protocol.md`](../First/04-protocol.md)。
