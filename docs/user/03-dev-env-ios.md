# 03 · 开发环境搭建 · iOS 发送端

iOS 端是音频发送端，采用「**Flutter 主 App + 原生 ReplayKit Broadcast Upload Extension**」架构：主 App 负责配对、发现、配置和广播引导；Broadcast Extension 负责系统音频采集、48kHz/Stereo/10ms PCM 归一化、Opus 编码、ChaCha20-Poly1305 加密和 UDP 发送。采集仅使用 Apple 官方 ReplayKit，**不使用私有 API / 越狱 / root**。

> iOS 开发只能在 macOS 上完成；ReplayKit 系统音频采集必须用真机验证，模拟器不能完成端到端验收。

## 1. 当前工程状态

- iOS 工程入口：`mobile/flutter_app/ios/Runner.xcworkspace`，不要直接打开 `Runner.xcodeproj`。
- 主 App target：`Runner`，Bundle ID 默认为 `com.soundlink.soundlink`。
- 采集扩展 target：`BroadcastExtension`，Bundle ID 默认为 `com.soundlink.soundlink.BroadcastExtension`。
- App Group：默认使用 `group.com.soundlink`，Runner 与 BroadcastExtension 必须完全一致。
- Opus：工程会在首次 Xcode 构建 BroadcastExtension 时从仓库内 libopus 源码生成 `mobile/flutter_app/ios/Frameworks/Opus.xcframework`；也可以手动执行脚本提前生成。

## 2. Mac 第一次准备

### 2.1 安装 Xcode

1. 打开 macOS 底部 Dock 的 **App Store**。
2. 左上角搜索框输入 `Xcode`。
3. 打开 Apple 官方 Xcode 页面，点击 **获取 / Get**，再点击 **安装 / Install**。
4. 安装完成后，在 Dock 点击 **启动台 / Launchpad**，打开 **Xcode**。
5. 首次打开会弹出组件安装窗口，点击 **Install**，输入 Mac 登录密码并等待完成。
6. 打开 Xcode 顶部菜单 **Xcode → Settings... → Locations**，确认 **Command Line Tools** 已选择当前 Xcode 版本。

### 2.2 安装命令行工具

打开 **终端 / Terminal**：按 `Command + 空格`，输入 `Terminal`，回车。执行：

```bash
xcode-select --install
sudo xcodebuild -license accept
xcodebuild -version
```

如果 `xcode-select --install` 提示已安装，是正常情况。

### 2.3 安装 Flutter 和 CocoaPods

先完成通用环境文档 [01-dev-env-common.md](./01-dev-env-common.md)，再在 Terminal 执行：

```bash
flutter doctor
sudo gem install cocoapods
pod --version
```

`flutter doctor` 里 iOS 相关项应尽量为绿色。如果提示 CocoaPods 缺失，重新打开一个 Terminal 再执行 `sudo gem install cocoapods`。

### 2.4 准备 iPhone

1. 用 USB 线把 iPhone 接到 Mac。
2. iPhone 弹出 **信任此电脑？** 时点击 **信任**，输入锁屏密码。
3. iPhone 打开 **设置 → 隐私与安全性 → 开发者模式**，打开开关并按提示重启。
4. 重启后 iPhone 会再次提示是否开启开发者模式，点击 **开启**。
5. Mac 上打开 Xcode，顶部菜单选择 **Window → Devices and Simulators**，左侧应能看到你的 iPhone。

## 3. 获取依赖与生成 Opus

进入 Flutter 工程目录：

```bash
cd /你的仓库路径/SoundLink/mobile/flutter_app
flutter pub get
cd ios
/bin/sh scripts/build_opus_xcframework.sh
pod install
```

`build_opus_xcframework.sh` 会在缺少源码时自动下载 Opus 1.5.2，并把源码放到 `mobile/flutter_app/android/app/src/main/cpp/opus`。如果你的内网 Mac 不能访问外网，请在能联网的机器下载 `https://downloads.xiph.org/releases/opus/opus-1.5.2.tar.gz`，解压后把 `opus-1.5.2` 目录重命名为 `opus`，放到上述路径。

完成后确认存在：

```bash
ls ios/Frameworks/Opus.xcframework
```

如果你已经在 `mobile/flutter_app/ios` 目录下，确认命令应改成：

```bash
ls Frameworks/Opus.xcframework
```

## 4. 打开 Xcode 工程

在 Terminal 从仓库根目录执行：

```bash
open mobile/flutter_app/ios/Runner.xcworkspace
```

Xcode 打开后注意左侧导航栏：

- 最左侧第一列是 **Project Navigator**，图标像文件夹；如果没显示，按 `Command + 1`。
- 左侧文件树最上方点击蓝色项目图标 **Runner**。
- 中间主区域左侧会出现 **PROJECT** 和 **TARGETS** 两组；后续主要点 **TARGETS** 下的 `Runner` 和 `BroadcastExtension`。
- 顶部工具栏左侧有运行按钮 **▶**；旁边是 scheme 和设备选择框。

## 5. 配置签名和 Bundle ID

### 5.1 登录 Apple 账号

1. Xcode 顶部菜单点击 **Xcode → Settings...**。
2. 打开上方 **Accounts** 标签。
3. 左下角点击 **+**，选择 **Apple Account**。
4. 登录你的 Apple ID。
5. 登录后关闭 Settings。

### 5.2 修改 Runner target

1. 左侧点击蓝色项目图标 **Runner**。
2. 中间 **TARGETS** 下点击 **Runner**。
3. 顶部标签点击 **Signing & Capabilities**。
4. 勾选 **Automatically manage signing**。
5. **Team** 下拉框选择你的 Apple 开发团队。
6. **Bundle Identifier** 建议改成全局唯一值，例如 `com.yourname.soundlink`。
7. 确认页面里有 **App Groups** 能力；如果没有，点击左上角 **+ Capability**，搜索 `App Groups` 并双击添加。
8. 在 App Groups 列表勾选或新增 `group.com.soundlink`。

### 5.3 修改 BroadcastExtension target

1. 同一页面中，**TARGETS** 下点击 **BroadcastExtension**。
2. 打开 **Signing & Capabilities**。
3. 勾选 **Automatically manage signing**。
4. **Team** 选择与 Runner 相同的团队。
5. **Bundle Identifier** 必须是 Runner Bundle ID 后追加 `.BroadcastExtension`，例如 `com.yourname.soundlink.BroadcastExtension`。
6. 确认有 **App Groups**，并且勾选与 Runner 完全相同的 `group.com.soundlink`。

> 如果你的 Apple 账号不允许创建固定的 `group.com.soundlink`，可以改成你自己的 App Group，例如 `group.com.yourname.soundlink`；但必须同时改 Runner、BroadcastExtension、`Runner/SoundLinkPlugin.swift` 和 `mobile/ios/BroadcastExtension/PairingStateReader.swift` 里的 App Group 字符串。

## 6. 真机编译运行主 App

### 6.1 在 Xcode 运行

1. Xcode 顶部工具栏中间的 scheme 选择 **Runner**。
2. scheme 右侧设备选择你的 iPhone，不要选择 Simulator。
3. 点击左上角 **▶ Run**，或按 `Command + R`。
4. 首次安装可能失败并提示签名、证书或设备信任问题，按 Xcode 红色错误里的 **Fix Issue** 或按下方故障排查处理。
5. App 安装到 iPhone 后，如果 iPhone 提示未信任开发者，打开 iPhone **设置 → 通用 → VPN 与设备管理**，点你的开发者账号，点击 **信任**。

### 6.2 用 Flutter 运行

先查看设备：

```bash
cd /你的仓库路径/SoundLink/mobile/flutter_app
flutter devices
flutter run -d <你的 iPhone 设备 id>
```

Flutter 适合调试主 App UI；如果要调 BroadcastExtension 的 Swift 代码，优先用 Xcode。

## 7. 真机调试 Broadcast Extension

ReplayKit Broadcast Extension 不是直接点扩展图标启动，它由系统广播选择器启动。

### 7.1 从 App 内启动广播选择器

1. 先启动桌面端 Receiver，并确认手机和电脑在同一局域网。
2. 在 iPhone 打开 SoundLink。
3. 在 App 内完成发现、配对、连接。
4. 点击 App 的开始采集/广播按钮后，会出现一个 Xcode 原生弹窗页面，标题为 **开始广播**。
5. 页面中间是系统广播按钮；点击它。
6. 系统弹窗中选择 **SoundLink BroadcastExtension**，点击 **开始广播**。
7. 倒计时结束后播放音乐；桌面端应听到声音。

### 7.2 从控制中心启动

1. iPhone 从右上角向下滑出 **控制中心**。
2. 找到 **屏幕录制** 圆点按钮。
3. 如果没有该按钮：打开 iPhone **设置 → 控制中心**，在列表里找到 **屏幕录制**，点左侧 **+** 添加。
4. 在控制中心 **长按** 屏幕录制按钮。
5. 在弹出的列表里选择 **SoundLink BroadcastExtension**。
6. 麦克风保持关闭，点击 **开始广播**。

### 7.3 在 Xcode 附加调试扩展

1. Xcode 先用 **Runner** scheme 安装并运行主 App。
2. iPhone 上按 7.1 或 7.2 启动广播。
3. Xcode 顶部菜单点击 **Debug → Attach to Process by PID or Name...**。
4. 输入 `BroadcastExtension`，选择匹配进程并点击 **Attach**。
5. 打开左侧 BroadcastExtension 源码文件，在行号左侧点击可设置断点。
6. 底部调试区如果没显示，点 Xcode 右上角调试区按钮，或按 `Command + Shift + Y`。

## 8. 常见问题

### 8.1 `Runner.xcodeproj` 和 `Runner.xcworkspace` 选哪个

必须打开 `Runner.xcworkspace`。CocoaPods 依赖只会正确挂到 workspace，直接打开 xcodeproj 经常出现 Pods 或 Flutter 依赖找不到。

### 8.2 `pod install` 报 `got PBXTargetDependency for attribute buildPhases`

这是 Xcode 工程文件里的 UUID 冲突导致的：同一个 UUID 不能同时表示 `PBXShellScriptBuildPhase` 和 `PBXTargetDependency`。当前仓库已修复；同步最新代码后在 `mobile/flutter_app/ios` 重新执行：

```bash
pod install
```

### 8.3 `build_opus_xcframework.sh` 报找不到 libopus 源码

当前脚本会自动下载 Opus 1.5.2。如果内网 Mac 无法访问外网，请在能联网的机器下载 `https://downloads.xiph.org/releases/opus/opus-1.5.2.tar.gz`，解压并重命名为 `opus`，放到：

```bash
mobile/flutter_app/android/app/src/main/cpp/opus
```

然后重新执行：

```bash
cd /你的仓库路径/SoundLink/mobile/flutter_app/ios
/bin/sh scripts/build_opus_xcframework.sh
```

### 8.4 `No such module 'Opus'`

执行：

```bash
cd /你的仓库路径/SoundLink/mobile/flutter_app/ios
/bin/sh scripts/build_opus_xcframework.sh
```

然后回到 Xcode，顶部菜单点击 **Product → Clean Build Folder**，再点 **▶ Run**。

### 8.5 `Signing for Runner requires a development team`

进入 **Runner 项目 → TARGETS → Runner → Signing & Capabilities**，选择 Team；再进入 **BroadcastExtension** target 选择同一个 Team。

### 8.6 `Bundle identifier is not available`

Bundle ID 被别人占用。把 Runner 改成你自己的唯一 ID，例如 `com.<你的英文名>.soundlink`，BroadcastExtension 改成同前缀加 `.BroadcastExtension`。

### 8.7 `App Groups` 报错或 App Group 无法创建

确认 Runner 和 BroadcastExtension 的 Team 一样；App Group 字符串完全一样；如果免费 Apple ID 无法创建 App Group，需要使用可用的 Apple Developer 账号或改用团队允许创建的 App Group。

### 8.8 iPhone 看不到 BroadcastExtension

确认你运行的是真机 Debug 包，不是模拟器；确认 BroadcastExtension target 已随 Runner 安装；重新运行 App 后再打开控制中心长按屏幕录制按钮。

### 8.9 本地网络发现不到电脑

首次启动 App 时 iOS 会弹出本地网络权限，必须点 **允许**。如果误点拒绝，打开 iPhone **设置 → 隐私与安全性 → 本地网络**，找到 SoundLink 并打开开关。

### 8.10 广播启动但桌面没声音

检查桌面 Receiver 是否已启动；手机和电脑是否同一 Wi-Fi；是否已完成配对；是否播放的是 DRM/受保护内容。受保护内容、系统通话和部分 App 音频可能无法被 ReplayKit 采集。

## 9. 验收清单

- `flutter doctor` 的 iOS 工具链无阻塞错误。
- `pod install` 成功。
- `ios/Frameworks/Opus.xcframework` 已生成。
- Xcode 中 Runner 和 BroadcastExtension 都选择了同一个 Team。
- Runner 和 BroadcastExtension 都启用了同一个 App Group。
- Xcode 选择真机后 `Runner` 可以 `Command + R` 安装启动。
- iPhone 控制中心能看到 SoundLink 的 BroadcastExtension。
- 开始广播并播放普通音乐后，桌面 Receiver 能听到声音。

## 10. 合规边界

- 只能使用 ReplayKit 广播扩展采集系统允许的音频。
- 不保证 DRM、受保护内容、系统通话或所有第三方 App 都可采集。
- 不使用私有 API，不要求越狱，不绕过 iOS 权限模型。
