# SoundLink

**LAN audio streaming for headphone users**: audio from your phone (iOS/Android) → local network → your computer's audio device. PC-to-PC streaming is also supported.

> Encrypted transport · LAN-only · No telemetry · Open source

[简体中文](README.md) · [License](LICENSE) · [Privacy Policy](docs/privacy.md) · [Contributing](CONTRIBUTING.md) · [Changelog](CHANGELOG.md)

---

## The problem it solves

You want to listen to music or videos playing on your phone through the headphones / DAC / speakers connected to your computer — but plugging your phone into that gear is inconvenient. SoundLink captures the audio currently playing on your phone, sends it over the local network, and plays it through your computer's audio device. Pair once, reconnect automatically.

**Good for**: music, long-form video.
**Not for**: gaming or voice chat (latency is not suitable); short-form video may show slight lag.

---

## Feature matrix

| Platform | Role | Status |
|---|---|---|
| Windows | Receiver (desktop playback) | ✅ Verified |
| Windows | Sender (WASAPI Loopback) | ✅ Verified |
| Android | Sender (MediaProjection) | ✅ Verified |
| macOS | Receiver (CoreAudio via cpal) | 🟡 Code ready, not verified |
| macOS | Sender (ScreenCaptureKit) | 🔴 Stub, not implemented |
| Linux | Receiver | 🔴 Not implemented |
| iOS | Sender (ReplayKit) | 🟡 Project ready, pending device testing |

> **Verified combinations**: Android phone → Windows PC, and Windows → Windows. Please do not assume other combinations work; help verifying them is very welcome (see [Contributing](CONTRIBUTING.md)).

Audio baseline: 48 kHz / stereo / Opus 10 ms / 128 kbps / 80 ms jitter buffer by default.
Runtime-adjustable: Opus bitrate, jitter buffer preset, desktop volume (sample rate / channels / frame size are fixed to the baseline).

---

## Quick start (Windows desktop)

### 1. Requirements

- Rust (stable, MSVC toolchain)
- Node.js 20 (see [`desktop/ui/.nvmrc`](desktop/ui/.nvmrc))
- CMake + a C compiler (needed to build vendored libopus 1.5)
- [Tauri 2 prerequisites](https://tauri.app/start/prerequisites/) (WebView2 Runtime ships with Windows 10+)

### 2. Clone and build

```powershell
git clone https://github.com/KversKv/SoundLink.git
cd SoundLink/desktop/ui
npm install                 # frontend deps + local Tauri CLI
npm run tauri:build:exe     # portable exe: desktop/src-tauri/target/release/soundlink.exe
```

For an NSIS installer (requires a global `cargo install tauri-cli`):

```powershell
cd SoundLink/desktop/src-tauri
cargo tauri build --features tauri_app
# artifacts under desktop/src-tauri/target/release/bundle/
```

### 3. Development mode

```powershell
cd SoundLink/desktop/src-tauri
cargo tauri dev --features tauri_app
```

> Production builds **must** enable the `tauri_app` feature, otherwise Opus decoding falls back to passthrough and produces noise. Do not build the GUI exe with `cargo build --release --features tauri_app` directly (it would load `localhost:1420` as in dev mode). Feature matrix and test commands: [`desktop/README.md`](desktop/README.md); full packaging guide: [`docs/user/05-build.md`](docs/user/05-build.md) (Simplified Chinese).

---

## Quick start (Android sender)

```bash
cd mobile/flutter_app
flutter pub get
flutter build apk --release -t lib/main.dart
# artifacts under build/app/outputs/flutter-apk/
```

Requires the Flutter SDK, Android SDK (minSdk 29), NDK + CMake (to build the libopus JNI layer). See [`docs/user/04-dev-env-android.md`](docs/user/04-dev-env-android.md).

iOS requires macOS + Xcode; run [`mobile/flutter_app/ios/scripts/build_opus_xcframework.sh`](mobile/flutter_app/ios/scripts/build_opus_xcframework.sh) first to produce the libopus XCFramework. See [`docs/user/03-dev-env-ios.md`](docs/user/03-dev-env-ios.md).

---

## How to use

1. On the computer, launch SoundLink, pick an output device, switch to the **Receiver** role and start receiving. An 8-digit pairing code appears.
2. On the phone, open the app, select the computer from the device list (discovered via mDNS), and enter the pairing code.
3. Android: tap "Start capture" and grant the system prompt. iOS: Control Center → long-press Screen Recording → choose SoundLink → start broadcasting.
4. Play any audio and it comes out of your computer. Pairing is persisted, so it reconnects next time.

Full user manual: [`docs/user/07-usage.md`](docs/user/07-usage.md); troubleshooting: [`docs/user/08-troubleshooting.md`](docs/user/08-troubleshooting.md) (both in Simplified Chinese for now — English docs are tracked as a launch task).

---

## Known limitations

- **LAN only**: no internet relay or NAT traversal. Router AP isolation / guest networks will block it.
- **DRM-protected content cannot be captured**: WASAPI Loopback / ReplayKit / MediaProjection are constrained by OS DRM policy; some streaming audio will be silent. SoundLink neither can nor tries to bypass this.
- **No global virtual sound card**: on iOS/Android only audio the OS permits capturing is captured; there is no silent background full capture.
- **Latency** is tuned for music and video. Wired / USB / 2.4 GHz low-latency headphones on the computer side are recommended.
- **Single receiver**: one sender maps to one receiver.
- **Desktop UI is Simplified Chinese only** for now (i18n is on the roadmap).
- **Installers are not code-signed**, so Windows SmartScreen will warn on first run.

---

## Repository layout

```
SoundLink/
├── mobile/             # Mobile sender (Flutter app + native capture)
│   ├── flutter_app/    # Main mobile project (build entry point)
│   ├── ios/            # iOS BroadcastExtension (Swift)
│   └── android/        # Early native Android structure (reference only)
├── desktop/            # Desktop (Tauri 2 + Rust + React/TS)
│   ├── src-tauri/      # Rust core (network/audio/pairing/device/config/log) + Tauri commands
│   └── ui/             # Frontend
├── shared/             # Cross-platform protocol & constants (single source)
└── docs/               # Design & user documentation
```

---

## Documentation

Design docs are written in Simplified Chinese. Key entry points:

| Topic | Entry |
|---|---|
| Top-level index | [`docs/First/SoundLinkStructrue.md`](docs/First/SoundLinkStructrue.md) |
| Architecture | [`docs/First/02-architecture.md`](docs/First/02-architecture.md) |
| Audio pipeline | [`docs/First/03-audio-pipeline.md`](docs/First/03-audio-pipeline.md) |
| Protocol spec | [`docs/First/04-protocol.md`](docs/First/04-protocol.md) |
| Pairing & security | [`docs/First/05-pairing-security.md`](docs/First/05-pairing-security.md) |
| Latency & experience | [`docs/First/06-latency-experience.md`](docs/First/06-latency-experience.md) |
| Platform compliance | [`docs/First/08-platform-notes.md`](docs/First/08-platform-notes.md) |
| Implementation spec | [`docs/First/11-implementation-spec.md`](docs/First/11-implementation-spec.md) |
| Plan & progress | [`docs/First/12-plan.md`](docs/First/12-plan.md) |
| Release readiness | [`docs/NewFunctions/release-readiness/00-release-overview.md`](docs/NewFunctions/release-readiness/00-release-overview.md) |
| Open-source launch backlog | [`docs/NewFunctions/opensource-launch/00-launch-overview.md`](docs/NewFunctions/opensource-launch/00-launch-overview.md) |
| User docs index | [`docs/user/00-index.md`](docs/user/00-index.md) |
| Privacy policy | [`docs/privacy.md`](docs/privacy.md) |

---

## Current status

- Phases 1/3/4 ✅ done: desktop receiver MVP, pairing & device discovery, UX polish.
- Phase 2 (mobile) 🟡 in progress: Android ✅ verified; iOS project ready, pending device testing.
- Phase 5 (desktop sender) 🟡 in progress: Windows WASAPI Loopback ✅ verified; macOS capture not implemented.
- Release readiness: P0 blockers ✅ done, P1 pre-beta hardening ✅ done, P2 optimizations 🟡 in progress.
- No public Release yet. Cross-platform completion (macOS/Linux) and UI i18n are pending.

Authoritative progress: [`docs/First/12-plan.md`](docs/First/12-plan.md). Launch backlog: [`docs/NewFunctions/opensource-launch/00-launch-overview.md`](docs/NewFunctions/opensource-launch/00-launch-overview.md).

---

## Tech stack

- Desktop: Tauri 2 + Rust (tokio) + React/TypeScript
- iOS: Swift + ReplayKit (capture) + Flutter (host app)
- Android: Kotlin + MediaProjection (capture) + Flutter (host app)
- Codec: libopus; transport: UDP (audio) + TCP (control); crypto: ChaCha20-Poly1305 / X25519 / Ed25519 / HKDF-SHA256

---

## Contributing

Issues and PRs are welcome. Dev environment, coding conventions, commit rules and verification commands: [`CONTRIBUTING.md`](CONTRIBUTING.md). Code of conduct: [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

**Most wanted help right now**: macOS/Linux verification and implementation, iOS device testing, Android device compatibility reports, English i18n.

Please do not report security vulnerabilities in public issues — follow [`SECURITY.md`](SECURITY.md).

---

## License

[MIT](LICENSE) · Copyright (c) 2026 KversKv

Third-party component licenses: [`docs/privacy.md`](docs/privacy.md) §6.

---

## Feedback

- Bugs / feature requests: [GitHub Issues](https://github.com/KversKv/SoundLink/issues)
- Usage discussion: [GitHub Discussions](https://github.com/KversKv/SoundLink/discussions)
