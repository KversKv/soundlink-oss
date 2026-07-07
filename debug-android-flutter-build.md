# Android Flutter Build Debug [CLOSED]

## Session

- Session ID: android-flutter-build
- Date: 2026-07-07
- Symptom: `flutter run -d 41091JEKB06514` fails during Gradle task `:app:compileFlutterBuildDebug` with Flutter process exit value `268435659`.
- Scope: Android debug build from `mobile/flutter_app`.

## Hypotheses

1. Flutter tool/frontend_server exits before producing kernel output due to stale `.dart_tool` or generated build artifacts.
2. Dart/Flutter compilation fails because generated plugin registrant or package config is inconsistent after Android/iOS project edits.
3. Gradle invokes Flutter with a different working directory or SDK state than the earlier `gradlew :app:assembleDebug` validation.
4. A Dart analyzer/compiler error exists that is not visible in the abbreviated Gradle output.
5. Windows path/tooling cache causes Flutter assemble to crash before Gradle can surface a normal Dart error.

## Evidence Log

- `flutter analyze` passed, excluding ordinary Dart analyzer failures.
- Pixel 8a is detected as Android arm64 device `41091JEKB06514`.
- Manual unquoted `flutter assemble ... -dTargetFile=lib/main.dart ...` failed because the frontend tried to read `package:soundlink/main` / `lib/main`.
- Manual quoted `flutter assemble ... -dTargetFile="lib/main.dart" ...` succeeded and used `package:soundlink/main.dart`.
- `flutter build apk --debug -t lib/main.dart` succeeded.
- `flutter run -d 41091JEKB06514 --target lib/main.dart --no-resident` succeeded.
- Project-level Gradle override of `FlutterTask.targetPath` made the original `flutter run -d 41091JEKB06514 --no-resident` succeed.

## Root Cause

Flutter Gradle task target resolution on this Windows toolchain could receive a truncated Dart entrypoint (`lib/main`), causing the Flutter frontend to compile `package:soundlink/main` instead of `package:soundlink/main.dart`.

## Fix

- Set Flutter Gradle DSL target to `lib/main.dart` in `android/app/build.gradle.kts`.
- Override all Flutter Gradle task `targetPath` values to `lib/main.dart` in `android/app/build.gradle.kts` so external Gradle properties cannot truncate the target path.

## Verification

- `flutter run -d 41091JEKB06514 --no-resident -v`: passed; APK installed and app launched on Pixel 8a.
- `flutter build apk --debug`: passed.
- `flutter analyze`: passed.
- `flutter test`: passed.

## Status

- Closed. No protocol, security, or audio business logic changed.
