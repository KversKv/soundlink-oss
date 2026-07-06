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

## 6. 常见问题排查

> 本节记录真机调试过程中实际遇到的问题与验证过的解决方法，按"环境问题 → 源码问题 → 依赖问题"顺序整理。所有命令在 Windows / PowerShell 环境验证通过。

### 6.1 `flutter doctor` 找不到 Android SDK

**问题现象**

```text
[X] Android toolchain - develop for Android devices
X Unable to locate Android SDK.
```

**原因**

Flutter 没有找到 Android SDK，可能是尚未安装 Android Studio / Android SDK，或 SDK 安装在自定义路径但未配置给 Flutter。

**解决方法**

安装 Android Studio，并通过 SDK Manager 安装 Android SDK。

默认 SDK 路径通常是：

```text
C:\Users\Administrator\AppData\Local\Android\Sdk
```

给 Flutter 指定 Android SDK：

```powershell
flutter config --android-sdk "C:\Users\Administrator\AppData\Local\Android\Sdk"
```

或：

```powershell
flutter config --android-sdk "$env:LOCALAPPDATA\Android\Sdk"
```

配置环境变量：

```powershell
$androidSdk="$env:LOCALAPPDATA\Android\Sdk"

[Environment]::SetEnvironmentVariable("ANDROID_HOME", $androidSdk, "User")
[Environment]::SetEnvironmentVariable("ANDROID_SDK_ROOT", $androidSdk, "User")
```

---

### 6.2 `Network resources` 检查失败

**问题现象**

```text
[!] Network resources
X A network error occurred while checking "https://maven.google.com/"
```

**原因**

当前网络无法直接访问 Google Maven / Android 相关资源。

**解决方法**

你有可用代理：

```text
192.168.3.231:909
```

临时配置 PowerShell 代理：

```powershell
$env:HTTP_PROXY="http://192.168.3.231:909"
$env:HTTPS_PROXY="http://192.168.3.231:909"
$env:NO_PROXY="localhost,127.0.0.1,::1"
```

永久配置用户环境变量：

```powershell
[Environment]::SetEnvironmentVariable("HTTP_PROXY", "http://192.168.3.231:909", "User")
[Environment]::SetEnvironmentVariable("HTTPS_PROXY", "http://192.168.3.231:909", "User")
[Environment]::SetEnvironmentVariable("NO_PROXY", "localhost,127.0.0.1,::1", "User")
```

Gradle 代理配置文件：

```text
C:\Users\Administrator\.gradle\gradle.properties
```

添加：

```properties
systemProp.http.proxyHost=192.168.3.231
systemProp.http.proxyPort=909
systemProp.https.proxyHost=192.168.3.231
systemProp.https.proxyPort=909
systemProp.http.nonProxyHosts=localhost|127.*|[::1]
```

---

### 6.3 缺少 Android SDK Command-line Tools

**问题现象**

```text
X cmdline-tools component is missing.
```

同时执行：

```powershell
sdkmanager --version
```

报错：

```text
sdkmanager : 无法将“sdkmanager”项识别为 cmdlet
```

**原因**

Android SDK Command-line Tools 未安装，或者没有加入环境变量 Path。

**解决方法**

在 Android Studio 中安装：

```text
Settings
→ Languages & Frameworks
→ Android SDK
→ SDK Tools
→ Android SDK Command-line Tools latest
```

安装后检查：

```powershell
$SDK="$env:LOCALAPPDATA\Android\Sdk"
Test-Path "$SDK\cmdline-tools\latest\bin\sdkmanager.bat"
```

正常应返回：

```text
True
```

将 SDK 工具加入用户 Path：

```powershell
$androidSdk="$env:LOCALAPPDATA\Android\Sdk"
$userPath=[Environment]::GetEnvironmentVariable("Path", "User")

$items=@(
    "$androidSdk\platform-tools",
    "$androidSdk\emulator",
    "$androidSdk\cmdline-tools\latest\bin"
)

foreach ($item in $items) {
    if ($userPath -notlike "*$item*") {
        $userPath="$userPath;$item"
    }
}

[Environment]::SetEnvironmentVariable("Path", $userPath, "User")
```

然后关闭 PowerShell，重新打开。

验证：

```powershell
sdkmanager --version
```

---

### 6.4 `JAVA_HOME is not set`

**问题现象**

```text
ERROR: JAVA_HOME is not set and no 'java' command could be found in your PATH.
```

**原因**

`sdkmanager` 需要 Java，但系统没有配置 `JAVA_HOME`，Path 中也找不到 `java.exe`。

**解决方法**

优先使用 Android Studio 自带 JBR：

```text
C:\Program Files\Android\Android Studio\jbr
```

临时测试：

```powershell
$env:JAVA_HOME="C:\Program Files\Android\Android Studio\jbr"
$env:Path="$env:JAVA_HOME\bin;$env:Path"

java -version
sdkmanager --version
```

确认有效后永久配置：

```powershell
[Environment]::SetEnvironmentVariable("JAVA_HOME", "C:\Program Files\Android\Android Studio\jbr", "User")

$javaBin="C:\Program Files\Android\Android Studio\jbr\bin"
$userPath=[Environment]::GetEnvironmentVariable("Path", "User")

if ($userPath -notlike "*$javaBin*") {
    $userPath="$javaBin;$userPath"
    [Environment]::SetEnvironmentVariable("Path", $userPath, "User")
}
```

重新打开 PowerShell 后验证：

```powershell
java -version
sdkmanager --version
```

---

### 6.5 Android Licenses 未接受

**问题现象**

```text
X Android license status unknown.
Run `flutter doctor --android-licenses`
```

**原因**

Android SDK 许可未接受。

**解决方法**

执行：

```powershell
flutter doctor --android-licenses
```

所有提示输入：

```text
y
```

完成后重新检查：

```powershell
flutter doctor
```

---

### 6.6 真机运行时 Kotlin 增量缓存跨盘错误

**问题现象**

运行：

```powershell
flutter run -d 41091JEKB06514
```

报错：

```text
Could not close incremental caches
this and base files have different roots:
C:\Users\Administrator\AppData\Local\Pub\Cache\...
and
D:\CodeProject\TRAE_Projects\SoundLink\mobile\flutter_app\android
```

**原因**

Flutter 项目在 D 盘，但 Pub 缓存在 C 盘。Kotlin 增量编译缓存遇到 Windows 跨盘路径时出错。

**解决方法**

将 Pub 缓存迁移到 D 盘：

```powershell
$pubCache="D:\.pub-cache"
New-Item -ItemType Directory -Force $pubCache | Out-Null

[Environment]::SetEnvironmentVariable("PUB_CACHE", $pubCache, "User")
$env:PUB_CACHE=$pubCache
```

关闭 IDE / PowerShell 后重新打开。

清理项目：

```powershell
cd "D:\CodeProject\TRAE_Projects\SoundLink\mobile\flutter_app"

cd android
.\gradlew.bat --stop
cd ..

flutter clean
Remove-Item -Recurse -Force ".\build" -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force ".\.dart_tool" -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force ".\android\.gradle" -ErrorAction SilentlyContinue

flutter pub get
```

确认依赖已经切到 D 盘：

```powershell
Select-String -Path ".\.dart_tool\package_config.json" -Pattern "shared_preferences_android" -Context 0,5
```

期望看到：

```text
"rootUri": "file:///D:/.pub-cache/hosted/pub.dev/shared_preferences_android-..."
```

同时建议在：

```text
android/gradle.properties
```

加入：

```properties
kotlin.incremental=false
```

---

### 6.7 Kotlin 源码 API 不兼容（SDK 36）

**问题现象**

`flutter run` 或 `flutter build apk --debug` 时 Gradle 报 `compileDebugKotlin` 失败：

```text
AudioCaptureService.kt: Too many arguments for 'constructor(): AudioPlaybackCaptureConfiguration'.
AudioCaptureService.kt: Unresolved reference 'captureAudioOutput'.
AudioCaptureService.kt: Unresolved reference 'setPerformanceMode'.
AudioCaptureService.kt: Unresolved reference 'PERFORMANCE_MODE_LOW_LATENCY'.
UdpAudioSender.kt: Unresolved reference 'ChaCha20Poly1305'.
UdpAudioSender.kt: Unresolved reference 'AEADMac'.
MainActivity.kt: Cannot access 'constructor(activity: ComponentActivity): SoundLinkPlugin': it is private.
```

**原因**

项目 `compileSdk = 36`（Android 16），多个旧 API 在 SDK 36 中被隐藏或移除：

- `AudioPlaybackCaptureConfiguration(MediaProjection)` 构造函数和 `captureAudioOutput()` 被隐藏，改用 `Builder`。
- `AudioRecord.Builder.setPerformanceMode()` 和 `AudioRecord.PERFORMANCE_MODE_LOW_LATENCY` 常量被移除。
- BouncyCastle `ChaCha20Poly1305` 类位于 `org.bouncycastle.crypto.modes`，而非 `engines`；`AEADMac` 类不存在。
- `SoundLinkPlugin` 构造函数声明为 `private`，但 `MainActivity` 需要外部实例化。

**解决方法**

#### 6.7.1 `AudioCaptureService.kt` — 改用 Builder

```kotlin
// 旧（SDK 36 已隐藏）：
val captureConfig = AudioPlaybackCaptureConfiguration(projection)
    .apply { captureAudioOutput(true) }

// 新（API 29+ 公开 API）：
val captureConfig = AudioPlaybackCaptureConfiguration.Builder(projection).build()
```

不指定 `addMatchingUsage` 即匹配所有可捕获播放（`USAGE_MEDIA` / `USAGE_GAME` / `USAGE_UNKNOWN`）。

#### 6.7.2 `AudioCaptureService.kt` — 删除 `setPerformanceMode`

```kotlin
// 旧（SDK 36 已移除）：
val record = AudioRecord.Builder()
    .setAudioPlaybackCaptureConfig(captureConfig)
    .setAudioFormat(audioFormat)
    .setBufferSizeInBytes(bufferSize)
    .setPerformanceMode(AudioRecord.PERFORMANCE_MODE_LOW_LATENCY)
    .build()

// 新（用 bufferSize 控制延迟）：
val record = AudioRecord.Builder()
    .setAudioPlaybackCaptureConfig(captureConfig)
    .setAudioFormat(audioFormat)
    .setBufferSizeInBytes(bufferSize)
    .build()
```

#### 6.7.3 `UdpAudioSender.kt` — 修正 BouncyCastle import

```kotlin
// 旧（路径错误 + 不存在的类）：
import org.bouncycastle.crypto.engines.ChaCha20Poly1305
import org.bouncycastle.crypto.macs.AEADMac

// 新：
import org.bouncycastle.crypto.modes.ChaCha20Poly1305
```

`build.gradle.kts` 依赖保持 `org.bouncycastle:bcprov-jdk18on:1.78.1`，无需引入 Tink。

#### 6.7.4 `SoundLinkPlugin.kt` — 去掉 `private`

```kotlin
// 旧：
class SoundLinkPlugin private constructor(
    private val activity: ComponentActivity,
) : MethodChannel.MethodCallHandler {

// 新：
class SoundLinkPlugin(
    private val activity: ComponentActivity,
) : MethodChannel.MethodCallHandler {
```

#### 6.7.5 `MainActivity.kt` — 使用 `FlutterFragmentActivity`

`SoundLinkPlugin` 需要 `ComponentActivity`，`FlutterFragmentActivity`（其子类）满足要求：

```kotlin
import io.flutter.embedding.android.FlutterFragmentActivity

class MainActivity : FlutterFragmentActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        val plugin = SoundLinkPlugin(this)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, SoundLinkPlugin.CHANNEL)
            .setMethodCallHandler(plugin)
    }
}
```

> 同时确认 `onMethodCall` 已加 `override` 修饰符。

---

### 6.8 libopus 未集成导致 CMake 编译失败

**问题现象**

Gradle 报 `buildCMakeDebug[arm64-v8a]` 失败：

```text
fatal error: 'opus/opus.h' file not found
   9 | #include <opus/opus.h>
     |          ^~~~~~~~~~~~~
```

**原因**

项目 JNI 代码 `opus_jni.c` 引用了 libopus，但 Android NDK 不自带 libopus，`CMakeLists.txt` 中的 `add_subdirectory(opus)` 被注释，且项目未提供 opus 源码或预编译库。

**解决方法**

采用源码集成方案（已验证）。

#### 6.8.1 下载 libopus 源码

```powershell
$cpp = "mobile\flutter_app\android\app\src\main\cpp"
$url  = "https://downloads.xiph.org/releases/opus/opus-1.5.2.tar.gz"
$gz   = "$cpp\opus-1.5.2.tar.gz"

Invoke-WebRequest -Uri $url -OutFile $gz
New-Item -ItemType Directory -Force -Path "$cpp\_extract" | Out-Null
tar -xzf $gz -C "$cpp\_extract"
if (Test-Path "$cpp\opus") { Remove-Item -Recurse -Force "$cpp\opus" }
Move-Item "$cpp\_extract\opus-1.5.2" "$cpp\opus"
Remove-Item -Recurse -Force "$cpp\_extract"
Remove-Item $gz
```

验证目录结构：

```text
mobile/flutter_app/android/app/src/main/cpp/opus/
├── CMakeLists.txt
├── include/
│   ├── opus.h
│   ├── opus_defines.h
│   └── ...
├── celt/
├── silk/
└── ...
```

#### 6.8.2 启用 `CMakeLists.txt` 中的 opus 构建

`mobile/flutter_app/android/app/src/main/cpp/CMakeLists.txt`：

```cmake
cmake_minimum_required(VERSION 3.22.1)
project(soundlink_opus)

# libopus 源码构建（静态库）。
# opus 1.5.2 默认 OPUS_DRED=OFF / OPUS_OSCE=OFF，不编译 DNN/ML 子系统，NDK 交叉编译无障碍。
add_subdirectory(opus)

add_library(soundlink_opus SHARED opus_jni.c)

target_link_libraries(soundlink_opus opus log)

target_include_directories(soundlink_opus PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/opus/include)
```

#### 6.8.3 修正 `opus_jni.c` 的 include 路径

opus 1.5.2 源码树中 `opus.h` 直接位于 `include/` 下（非 `include/opus/opus.h`）：

```c
// 旧：
#include <opus/opus.h>

// 新：
#include <opus.h>
```

#### 6.8.4 验证

```powershell
flutter build apk --debug
```

构建日志应看到 opus 子项目编译（约 149 个 C 文件），最终输出：

```text
Built build\app\outputs\flutter-apk\app-debug.apk
```

> 备选方案：若只需先跑通 App 而暂不需要 Opus 编码，可临时注释 `build.gradle.kts` 中的 `externalNativeBuild { cmake { path = ... } }` 块，并注释 `OpusEncoder.kt` 中的 `System.loadLibrary("soundlink_opus")`。但采集功能将不可用。

---

### 6.9 运行时 mDNS SocketException（不影响主流程）

**问题现象**

`flutter run` 启动后日志中出现 mDNS 异常：

```text
E/Dart: Dart Socket ERROR: `reusePort` not supported on this platform.
E/flutter: SocketException: Send failed (OS Error: Network is unreachable, errno = 101),
           address = 0.0.0.0, port = 5353
```

**原因**

`multicast_dns` 包在 Android 上使用 `SO_REUSEPORT`，部分设备不支持；同时设备可能未连接 WiFi 或未持有 `MulticastLock`。

**解决方法**

此错误不影响 App 启动与采集功能，仅影响 mDNS 设备发现。如需设备发现正常工作：

1. 确认设备已连接到与电脑同一局域网 WiFi。
2. 在 `AndroidManifest.xml` 添加权限：

```xml
<uses-permission android:name="android.permission.CHANGE_WIFI_MULTICAST_STATE" />
```

3. 在采集 Service 或 MainActivity 中申请 `WifiManager.MulticastLock`。

> 第一版可通过手动输入接收端 IP 地址绕过 mDNS 发现，详见 [`06-debug.md`](./06-debug.md)。

---

### 6.10 总体状态

**已解决的问题**：

- Flutter / Android SDK / JDK 17 / Android Studio 环境就绪
- 网络代理配置完成，`flutter doctor` 全绿
- Pub Cache 迁移到 D 盘，Kotlin 跨盘缓存问题消除
- Kotlin 源码 SDK 36 API 不兼容问题修复（6.7）
- libopus 源码集成完成，CMake 编译通过（6.8）
- `flutter build apk --debug` 成功输出 APK
- `flutter run -d 41091JEKB06514` 真机调试启动成功

**验证结果**（2026-07-06）：

```text
flutter build apk --debug      # ✓ Built build\app\outputs\flutter-apk\app-debug.apk
flutter run -d 41091JEKB06514  # ✓ Dart VM Service on Pixel 8a available
```

**待后续处理**：

- mDNS 设备发现的 Android 兼容性（6.9，非阻塞）
- 真机 MediaProjection 授权 + 实际采集链路验证
- Opus 编码 + UDP 加密发送的端到端真机测试