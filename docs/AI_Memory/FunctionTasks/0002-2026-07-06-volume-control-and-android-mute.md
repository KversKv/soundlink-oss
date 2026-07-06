<!-- FT-0002 -->
# 桌面端音量控制 + Android 端自动静音归档（2026-07-06）

> 场景：用户已跑通 Android ↔ 桌面端音频转发，但遇到三个体验问题：
> 1. 手机扬声器仍在播放，希望转发后手机静音；
> 2. 手机音量调 0 后电脑端仍正常（疑问行为）；
> 3. 电脑端如何调节音量。
>
> 本次会话完成桌面端软件音量 + UI 滑块、Android 端自动静音辅助类，并解答了 AudioPlaybackCapture 的「音量前采集」特性。

---

## 一、需求与方案

### 问题 1：手机扬声器静音
- **根因**：`AudioPlaybackCapture` 是镜像采集，不阻止原应用播放。
- **方案 B**：采集时 `AudioManager.setStreamVolume(STREAM_MUSIC, 0, 0)`，停止时恢复。
- **关键依据**：采集的是音量调节**前**的 PCM，音量 0 不影响转发。

### 问题 2：手机音量 0 电脑仍正常
- **结论**：**特性而非 bug**。AudioFlinger 把应用 PCM 同时分两路：一路经音量调节到扬声器，一路到 AudioPlaybackCapture（音量前）。所以音量 0 阻断扬声器但不阻断采集。
- **副作用**：这正是问题 1 方案 B 成立的基础。
- **风险**：部分定制 ROM（华为/小米老版本）可能改了 AudioFlinger，导致音量 0 时采集也静音。需现场测试。

### 问题 3：电脑端音量
- 当前代码无音量控制（PCM 直喂 cpal）。
- **方案 B（推荐）**：软件音量，在 `PlaybackSource::fill` 后对 i16 PCM 做标量乘法 + clip。

---

## 二、实现清单

### 桌面端软件音量 + UI 滑块

| 文件 | 改动 |
|---|---|
| [audio/output/mod.rs](file:///d:/CodeProject/TRAE_Projects/SoundLink/desktop/src-tauri/src/audio/output/mod.rs) | 新增 `VolumeControl`（AtomicU32 存 f32 bits，回调线程无锁读）；`AudioOutput` 加 `volume` 字段 + `set_volume`/`volume` 方法；`build_stream` 在 i16 PCM 阶段统一应用增益（带 `.clamp(-32768.0, 32767.0)` 防 clip） |
| [receiver.rs](file:///d:/CodeProject/TRAE_Projects/SoundLink/desktop/src-tauri/src/receiver.rs) | `ReceiverEngine::set_volume` / `volume` 转发给 `AudioOutput` |
| [commands/mod.rs](file:///d:/CodeProject/TRAE_Projects/SoundLink/desktop/src-tauri/src/commands/mod.rs) | 新增 `set_volume(volume: f32) -> f32` / `get_volume() -> f32` Tauri 命令 |
| [main.rs](file:///d:/CodeProject/TRAE_Projects/SoundLink/desktop/src-tauri/src/main.rs) | 注册 `set_volume` / `get_volume` 命令 |
| [App.tsx](file:///d:/CodeProject/TRAE_Projects/SoundLink/desktop/ui/src/App.tsx) | 接收模式面板新增「音量」滑块（0–100%），onChange 实时调用 `set_volume` |

### Android 端自动静音

| 文件 | 改动 |
|---|---|
| [VolumeMuteController.kt](file:///d:/CodeProject/TRAE_Projects/SoundLink/mobile/android/app/src/main/java/com/soundlink/capture/VolumeMuteController.kt) | 新建。封装 `muteMediaVolume()` / `restoreMediaVolume()`，幂等，处理 `savedVolume = -1` 边界，含详细集成指引注释 |
| [AudioCaptureService.kt](file:///d:/CodeProject/TRAE_Projects/SoundLink/mobile/android/app/src/main/java/com/soundlink/capture/AudioCaptureService.kt) | 占位注释里加集成示例（onStartCommand 调 mute、onDestroy/onTaskRemoved 调 restore） |

---

## 三、关键设计决策

### 3.1 VolumeControl 用 AtomicU32 存 f32::to_bits

```rust
pub struct VolumeControl(Arc<AtomicU32>);
impl VolumeControl {
    pub fn set(&self, v: f32) {
        self.0.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}
```

- **为什么不用 Mutex**：cpal 回调是实时线程，加锁会引入不确定性。
- **为什么不用 AtomicF32**：Rust 标准库没有 AtomicF32，用 `to_bits`/`from_bits` 模拟。
- **Relaxed 序序**：音量不需要跨线程同步顺序，单值读写 Relaxed 足够。

### 3.2 增益在 i16 阶段统一应用

```rust
if vol != 1.0 {
    for s in tmp.iter_mut() {
        *s = (*s as f32 * vol).clamp(-32768.0, 32767.0) as i16;
    }
}
```

- **为什么在 i16 阶段**：cpal 回调支持 I16/F32/U16 三种采样格式，统一在 i16 阶段算一次，避免三种格式重复写增益逻辑。
- **vol == 1.0 跳过循环**：零开销快路径。
- **clip 必须做**：i16 范围 [-32768, 32767]，不 clip 会回绕失真。

### 3.3 VolumeMuteController 幂等性

```kotlin
fun muteMediaVolume() {
    if (savedMusicVolume != NOT_SAVED) return  // 已静音过，跳过
    val cur = audioManager.getStreamVolume(AudioManager.STREAM_MUSIC)
    if (cur <= 0) return  // 已经是 0，不保存，避免退出时把 0 误恢复成其他值
    savedMusicVolume = cur
    audioManager.setStreamVolume(AudioManager.STREAM_MUSIC, 0, 0)
}

fun restoreMediaVolume() {
    if (savedMusicVolume == NOT_SAVED) return  // 未保存过，no-op
    audioManager.setStreamVolume(AudioManager.STREAM_MUSIC, savedMusicVolume, 0)
    savedMusicVolume = NOT_SAVED
}
```

- **幂等**：多次 mute / 多次 restore 都安全。
- **cur == 0 边界**：用户当前音量已是 0 时不保存，否则退出时会把 0 误恢复成其他值。
- **NOT_SAVED = -1**：用 -1 而非 0 标记未保存，避免和合法音量值 0 混淆。

### 3.4 异常崩溃恢复

Service 被系统杀掉时不走 `onDestroy`，静音无法自动恢复。建议：
1. `onDestroy` + `onTaskRemoved` 双保险
2. 主 App 进入前台时调一次 `restoreMediaVolume()`（幂等）

---

## 四、验证

```powershell
cd desktop\src-tauri
cargo test --lib --no-default-features
# 45 passed; 0 failed

cargo clippy --features tauri_app --no-default-features
# 2 warnings（与本次改动无关，是 Rust 1.96 新版 clippy 对 Role enum 的旧代码告警）
```

桌面端 UI 验证：`cargo tauri dev --features tauri_app`（注：项目 Cargo.toml 已把 `tauri_app` 依赖 `opus`，无需手动加 `--features opus`，详见 [FT-0001](./0001-2026-07-06-audio-noise-debug.md) 坑 9）。

---

## 五、用户需自行完成的部分

### Android 端集成（用户本地有自己的 AudioCaptureService.kt 实现）

按 [VolumeMuteController.kt](file:///d:/CodeProject/TRAE_Projects/SoundLink/mobile/android/app/src/main/java/com/soundlink/capture/VolumeMuteController.kt) 顶部注释示例嵌入：

```kotlin
private val mute = VolumeMuteController(this)

override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    startForeground(...)                  // 先确保前台通知已展示
    mute.muteMediaVolume()                // ← 启动采集前
    // ... 启动 MediaProjection + AudioPlaybackCapture
}

override fun onDestroy() {
    // ... 停止采集
    mute.restoreMediaVolume()             // ← 必须恢复
    super.onDestroy()
}

override fun onTaskRemoved(rootIntent: Intent?) {
    mute.restoreMediaVolume()             // ← 用户划掉任务时也恢复
    super.onTaskRemoved(rootIntent)
}
```

---

## 六、已知边界

1. **定制 ROM 风险**：部分华为/小米老版本 ROM 可能 STREAM_MUSIC=0 时也阻断采集。若发现此问题，改用「插入耳机」方案。
2. **崩溃恢复**：Service 进程被系统杀掉而非正常 onDestroy 时，静音无法自动恢复。建议主 App 进入前台时调一次 `restoreMediaVolume()`。
3. **采集期间用户手动调音量**：当前实现会恢复到 mute 前保存的值，不保留用户采集期间的手动调整。第一版取舍，优先保证「恢复到非 0」。
4. **iOS 不适用**：iOS ReplayKit 不存在此问题（系统不会同时输出扬声器），本方案仅 Android。

---

## 七、关键文件索引

| 文件 | 作用 |
|---|---|
| `desktop/src-tauri/src/audio/output/mod.rs` | cpal 输出 + VolumeControl 软件音量 |
| `desktop/src-tauri/src/receiver.rs` | ReceiverEngine.set_volume 转发 |
| `desktop/src-tauri/src/commands/mod.rs` | set_volume / get_volume Tauri 命令 |
| `desktop/src-tauri/src/main.rs` | 命令注册 |
| `desktop/ui/src/App.tsx` | 音量滑块 UI |
| `mobile/android/app/src/main/java/com/soundlink/capture/VolumeMuteController.kt` | Android 自动静音辅助类 |
| `mobile/android/app/src/main/java/com/soundlink/capture/AudioCaptureService.kt` | 集成示例注释（占位） |

---

## 八、关联文档

- [FT-0001 音频杂音调试实录](./0001-2026-07-06-audio-noise-debug.md)：杂音问题修复后才有本次音量控制需求。坑 9 修复了 `tauri_app` feature 不依赖 opus 的问题，本次 `cargo tauri dev --features tauri_app` 已能直接出声。
