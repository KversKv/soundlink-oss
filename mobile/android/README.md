# mobile/android — 早期原生结构参考（非构建入口）

> ⚠️ **此目录不参与构建**。Android 实际构建入口是 [`mobile/flutter_app/android`](../flutter_app/android)，采集实现在 `mobile/flutter_app/android/app/src/main/kotlin/com/soundlink/soundlink/SoundLinkPlugin.kt`，Opus JNI 在 `.../main/cpp/opus_jni.c`。

保留原因：这里的 Kotlin 文件是阶段 2 早期的模块拆分草稿，作为分层参考与后续可能的纯原生实现起点。

包根：`com.soundlink`

## 目录草稿

- `app/src/main/java/com/soundlink/capture/` — MediaProjection + AudioPlaybackCapture + AudioRecord 采集、静音控制
- `.../codec/` — Opus 编码封装
- `.../discovery/` — NSD/mDNS 发现桌面端
- `.../network/` — UDP 音频发送
- `.../pairing/` — 配对码、密钥协商、信任存储
- `.../ui/` — Compose 界面草稿

## 合规要点（对实际实现同样有效）

需前台 Service（`mediaProjection` 类型）+ 用户授权弹窗 + 通知栏状态；API 29+ 起部分应用/受保护内容不可采（应用可通过 `setAllowedCapturePolicy` 拒绝被采集）。详见 `docs/First/08-platform-notes.md`。

## 构建 Android APK

```bash
cd mobile/flutter_app
flutter pub get
flutter build apk --release -t lib/main.dart
```

环境要求见 `docs/user/04-dev-env-android.md`。
