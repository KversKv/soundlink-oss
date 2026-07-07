<!-- FT-0005 -->

# Android Flutter Run TargetPath 修复实录（2026-07-07）

> 场景：Pixel 8a 真机执行 `flutter run -d 41091JEKB06514` 时，Gradle `:app:compileFlutterBuildDebug` 失败，Flutter 进程退出码为 `268435659`。

## 背景

- 入口目录：`mobile/flutter_app`。
- 设备：Pixel 8a，device id `41091JEKB06514`，Android arm64。
- 现象：普通 `flutter run` 在 Gradle 编译 Flutter bundle 阶段失败，简略输出没有暴露 Dart 编译真实错误。

## 根因分析

| 证据 | 结论 |
|---|---|
| `flutter analyze` 通过 | 排除普通 Dart 静态错误 |
| 未加引号的手动 `flutter assemble ... -dTargetFile=lib/main.dart ...` 失败 | frontend_server 实际读取 `package:soundlink/main` / `lib/main` |
| 带引号的手动 `flutter assemble ... -dTargetFile="lib/main.dart" ...` 成功 | Dart 入口恢复为 `package:soundlink/main.dart` |
| `flutter run --target lib/main.dart --no-resident` 成功 | 显式 target 可绕过入口截断问题 |
| 覆盖 Gradle `FlutterTask.targetPath` 后原始 `flutter run -d 41091JEKB06514 --no-resident` 成功 | 项目侧修复可消除复现 |

## 实现清单

| 文件 | 修改 |
|---|---|
| `mobile/flutter_app/android/app/build.gradle.kts` | 导入 `com.flutter.gradle.tasks.FlutterTask` |
| `mobile/flutter_app/android/app/build.gradle.kts` | 在 `flutter {}` 中显式设置 `target = "lib/main.dart"` |
| `mobile/flutter_app/android/app/build.gradle.kts` | 对所有 `FlutterTask` 覆盖 `targetPath = "lib/main.dart"` |
| `docs/First/12-plan.md` | 追加 Pixel 8a 真机安装启动通过记录，阶段端到端验收仍保持未勾选 |
| `docs/First/11-implementation-spec.md` | 补充 Android 构建验证基线与 Windows/Flutter 3.44 targetPath 约束 |
| `debug-android-flutter-build.md` | 记录调试证据、根因、修复与验证结果，状态关闭 |

## 关键设计决策

- 不修改 Flutter SDK，避免把机器级 workaround 带入工具链。
- 不改 Dart 业务逻辑、协议、安全、音频链路，仅修正 Android Gradle 构建入口解析。
- 不将阶段 2 Android 端到端标为完成；本次只证明 APK 构建、安装、启动闭环已通过，MediaProjection 授权与桌面播放仍待实测。

## 验证结果

- `flutter run -d 41091JEKB06514 --no-resident -v`：通过，APK 安装成功，App 启动并连接 VM Service。
- `flutter build apk --debug`：通过，生成 `build/app/outputs/flutter-apk/app-debug.apk`。
- `flutter analyze`：通过，No issues found。
- `flutter test`：通过，All tests passed。

## 已知边界

- 当前验证到真机安装启动；尚未完成 MediaProjection 授权、AudioPlaybackCapture 实际采集、UDP 发送到桌面端播放的完整端到端验收。
- `debug-android-flutter-build.md` 暂保留作为调试过程记录；若后续确认不再需要，可按会话归档策略清理或保留。

## 关联文档

- [FT-0004](./0004-2026-07-07-mobile-build-and-plan-sync.md)
- `docs/First/12-plan.md`
- `docs/First/11-implementation-spec.md`
