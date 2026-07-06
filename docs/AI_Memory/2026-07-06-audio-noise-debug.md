# 音频杂音调试实录（2026-07-06）

> 场景：Android 手机（Pixel 8a）采集屏幕音频 → Opus 编码 → UDP 加密传输 → 桌面端（Windows）解密 → Opus 解码 → cpal 输出。
> 故障现象：手机播放 1kHz 测试音，电脑端耳机里听到的是**全程杂音**（白噪声特征，无 1kHz 主频）。
> 调试跨越 5 轮对话，最终根因有三个层次，层层嵌套。

---

## 一、踩坑总览

| # | 坑点 | 文件 | 根因 | 修复 |
|---|---|---|---|---|
| 1 | APP 闪退 | `AudioCaptureService.kt` | `AudioPlaybackCaptureConfiguration.Builder` 未加 matching usage 规则，`AudioMixingRule.build()` 抛 `IllegalArgumentException` | 显式 addUsage USAGE_MEDIA/GAME/UNKNOWN |
| 2 | `reusePort not supported` | `discovery_service.dart` | `multicast_dns` 默认 `reusePort: true`，Android 不支持 | 注入 `rawDatagramSocketFactory` 强制 `reusePort: false` |
| 3 | `ClassCastException` | `SoundLinkPlugin.kt` | Flutter 端传 JSON 字符串，Kotlin 端按 Map 取值 | 直接 `call.argument<String>("config")` |
| 4 | cpal "只能在创建线程启动" | `audio/output/mod.rs` | `AudioOutput::new()` 记录 owner_thread，但 `start()` 在 tokio worker 调用 | 移除 owner_thread 检查（cpal 0.15 Stream 是 Send） |
| 5 | TimeoutException 掩盖断连 | `control_client.dart` | `waitFor` 缺 `onDone`，socket 断开时 completer 永不完成 | 加 `onDone` 立即抛 StateError |
| 6 | **重采样器 lerp 用错样本** ⭐ | `audio/resampler.rs` | `lerp(last, cur, frac=0) = last`，导致输出全是上一帧末样本 | 改为 `lerp(cur, next, frac)` |
| 7 | **OPUS_APPLICATION_AUDIO 常量值错误** ⭐⭐ | `audio/opus_codec.rs` | 写成 `24953`（正确值 `2049`），libopus 初始化失败回退 PassthroughCodec | 改回 `2049` |
| 8 | **欠流时调 decode_plc 污染解码器状态** ⭐⭐ | `receiver.rs` | `PopResult::Empty` 分支调 `decode_plc()` 推进 Opus 解码器内部状态，真实帧到达时解码变噪声 | Empty 改为返回静音 |
| 9 | **tauri_app feature 不依赖 opus** ⭐⭐⭐ | `Cargo.toml` | `cargo tauri dev --features tauri_app` 不带 opus → 回退 PassthroughCodec | `tauri_app` 依赖链加入 `opus` |
| 10 | DebugDumper 阻塞 cpal 实时线程 | `receiver.rs` | 文件 I/O 在音频回调线程 | 改为 mpsc 异步队列 + 独立 IO 线程 |

带 ⭐ 的是导致"杂音"的直接原因（共 4 个，层层嵌套）。

---

## 二、调试方法论（重点）

### 2.1 分阶段 dump 是黄金标准

在接收链路各阶段加 dump 点，**对比各阶段的频谱/自相关**，能精确定位故障环节：

```
Android 采集 PCM → Android Opus 编码 → [网络] → 桌面端 Opus 解码 → 桌面端重采样 → cpal 输出
        ①              ②                          ③                  ④              ⑤
```

本次在 ①②③④⑤ 全部加了 dump，用 Python + numpy 做频谱分析：

```python
# 关键诊断代码：FFT + 自相关
fft = np.abs(np.fft.rfft(seg * np.hanning(seg.size)))
freqs = np.fft.rfftfreq(seg.size, 1/48000)
ac48 = np.corrcoef(seg[:-48], seg[48:])[0, 1]  # 1kHz @ 48k → lag 48
```

- 干净 1kHz 正弦：FFT 主峰 1000Hz，自相关 lag48 ≈ 1.0
- 噪声：FFT 主峰在 19-22kHz，自相关 lag48 ≈ 0

### 2.2 "同样的数据，独立测试 OK，集成测试 NG" → 状态污染

本次最关键的诊断：用桌面端接收的 Opus dump 喂给 example 程序解码 → **干净正弦**；同样的数据在 receiver 里解码 → **噪声**。

这指向 **receiver 的解码器状态被污染**。最终定位到 `PopResult::Empty` 分支调 `decode_plc()` 推进了解码器状态。

**教训**：Opus 解码器是有状态的，PLC 调用会推进内部状态。**欠流（Empty）≠ 丢包（Lost）**，欠流应返回静音，不能用 PLC。

### 2.3 PassthroughCodec 是危险的后门

`default_codec()` 在 libopus 初始化失败时回退 `PassthroughCodec`，它把 PCM 字节原样当 Opus 帧。这会让"编码-解码"看起来 roundtrip 正常（因为 encode 也是原样），但跨端通信时格式完全不匹配 → 噪声。

**教训**：
- PassthroughCodec 只能用于无 libopus 时的链路自测，**绝不能进入生产路径**
- `default_codec()` 的回退警告要用 `tracing::warn!` 但**默认 tracing subscriber 不初始化时看不到** → 应该 panic 或 log 到 stderr
- feature gate 要把生产路径强制依赖 opus

### 2.4 rustc ICE 不是你的错

调试过程中遇到两次 `rustc 1.96.1` 编译器 internal compiler error (ICE)：
```
error: the compiler unexpectedly panicked. This is a bug.
```

这是 incremental compilation 缓存损坏导致的。`cargo clean -p soundlink` 清理后即可。**不要花时间怀疑自己的代码**。

---

## 三、各坑点详细分析

### 坑 6：重采样器 lerp 用错样本

`DriftResampler::process()` 的线性插值：

```rust
// ❌ 错误：frac=0 时 lerp(last, cur, 0) = last，输出全是上一帧末样本
let l = lerp_i16(self.last_l, cur_l, frac);

// ✅ 正确：frac=0 时 lerp(cur, next, 0) = cur
let l = lerp_i16(cur_l, next_l, frac);
```

**症状**：即使 ratio=1.0（无漂移校正），输出也是上一帧最后一个样本重复 480 次 → 直流偏置 + 残留 → 完全杂音。

**回归测试**：
```rust
#[test]
fn process_ratio_one_is_identity() {
    let mut r = DriftResampler::new();
    r.ratio = 1.0;
    let input: Vec<i16> = (0..960).map(|i| (i * 2) as i16).collect();
    let out = r.process(&input);
    assert_eq!(out, input);  // ratio=1.0 必须严格 identity
}
```

### 坑 7：OPUS_APPLICATION_AUDIO 常量值错误 ⭐⭐

```rust
// ❌ 错误：24953 不是合法 application 值
const OPUS_APPLICATION_AUDIO: c_int = 24953;

// ✅ 正确：libopus 官方值
const OPUS_APPLICATION_AUDIO: c_int = 2049;
```

合法值（来自 `opus_defines.h`）：
- `OPUS_APPLICATION_VOIP` = 2048
- `OPUS_APPLICATION_AUDIO` = **2049**
- `OPUS_APPLICATION_RESTRICTED_LOWDELAY` = 2051

**故障链**：
1. `opus_encoder_create(48000, 2, 24953, &err)` → 非法参数
2. libopus 返回 `err = -1` (OPUS_BAD_ARG)，encoder = null
3. `LibopusCodec::new()` 返回 `Err(Create(-1))`
4. `default_codec()` 回退 `PassthroughCodec`（但 `tracing::warn!` 在 example 里没初始化 subscriber 看不到）
5. 桌面端用 PassthroughCodec 把 Android 端真实 Opus 字节按 i16 解析 → 噪声

**验证方法**：对比 Android 端和桌面端编码出的 Opus 帧 TOC 字节：
- Android: `0xf4` (config=30, stereo=1, count=0) — 真正压缩，帧长 161-252 字节
- 桌面端（修复前）: `0xfb` (config=31, stereo=0, count=3) — PassthroughCodec 输出，帧长 1920 字节（= PCM 原样）

**教训**：手写 FFI 常量值时，**必须从官方头文件复制**，不要凭记忆。libopus_sys crate 已经导出了这些常量，应该直接用 `opusffi::OPUS_APPLICATION_AUDIO` 而不是自己定义。

### 坑 8：欠流时 decode_plc 污染解码器状态 ⭐⭐

```rust
// ❌ 错误：Empty 时调 decode_plc
PopResult::Empty => {
    self.codec.lock().decode_plc()  // PLC 推进解码器状态
}

// ✅ 正确：Empty 返回静音
PopResult::Empty => {
    vec![0i16; frame_pcm_len()]
}
```

**诊断过程**：
1. 独立 example 程序解码桌面端 Opus dump → 干净 1kHz 正弦（FFT 1000Hz，自相关 1.0）
2. receiver 实际解码同样数据 → 噪声（FFT 22800Hz，自相关 -0.0158）
3. Opus dump 2113 帧 sequence 0-2112 完全连续，0 丢包
4. resampled dump 2227 帧 → 多出 114 帧是 Empty（欠流）
5. 114 次 `decode_plc()` 调用推进了解码器状态 → 真实帧到达时解码变噪声

**关键认知**：
- `PopResult::Empty`（欠流）≠ `PopResult::Lost`（丢包）
- 欠流是缓冲耗尽，下一帧还在路上，**不该推进解码器**
- 丢包是 sequence 跳跃，**需要 PLC 填补**
- Opus 解码器有状态，PLC 调用会改变内部状态

### 坑 9：tauri_app feature 不依赖 opus ⭐⭐⭐

```toml
# ❌ 之前：tauri_app 不依赖 opus
tauri_app = ["dep:tauri", "dep:tauri-build", "dep:dirs", "dep:tauri-plugin-opener"]

# ✅ 现在：tauri_app 自动启用 opus
tauri_app = ["dep:tauri", "dep:tauri-build", "dep:dirs", "dep:tauri-plugin-opener", "opus"]
```

**故障现象**：
- `cargo tauri dev --features tauri_app` → 杂音（未启用 opus → PassthroughCodec）
- `cargo tauri dev --features tauri_app opus` → 完美音频

这是坑 7 的"余波"：坑 7 修复后，用 `--features opus` 的 example 测试都正常，但 `cargo tauri dev` 命令里没加 `opus` → 仍然回退 PassthroughCodec。

**教训**：
- 生产路径的 feature 必须强制依赖 opus
- `default_codec()` 的回退逻辑在生产构建中应该 panic 而不是 warn
- 文档/README 里要明确写 `cargo tauri dev --features tauri_app`（不需要手动加 opus）

### 坑 10：DebugDumper 阻塞 cpal 实时线程

```rust
// ❌ 错误：在 cpal 回调线程里直接写文件
fn dump_pcm_decoded(&mut self, pcm: &[i16]) {
    let _ = self.pcm_decoded_file.write_all(&bytes);  // 阻塞！
}

// ✅ 正确：mpsc 异步队列，回调线程只 send
fn dump_pcm_decoded(&self, pcm: &[i16]) {
    let _ = self.tx.send(DumpMsg::PcmDecoded(pcm.to_vec()));  // 非阻塞
}
// 独立 IO 线程从队列取消息写文件
```

**症状**：开启 SOUNDLINK_DUMP=1 后桌面端崩溃重启。

**教训**：cpal 回调是**实时线程**，任何阻塞 I/O（文件、锁、内存分配）都可能导致回调超时，触发 WASAPI 强制停流或进程崩溃。实时线程里只能做：无锁队列 send、预分配缓冲区读写。

---

## 四、Android 端调试文件获取

### 4.1 MediaStore 写公共 Download 目录

Android 10+ 不需要存储权限，用 MediaStore API：

```kotlin
private fun openMediaStoreFile(fileName: String): OutputStream {
    val collection = MediaStore.Files.getContentUri("external")
    // 先删除旧文件
    contentResolver.delete(collection,
        "${MediaStore.MediaColumns.RELATIVE_PATH} = ? AND ${MediaStore.MediaColumns.DISPLAY_NAME} = ?",
        arrayOf("${Environment.DIRECTORY_DOWNLOADS}/soundlink_dump/", fileName))
    // 插入新文件
    val values = ContentValues().apply {
        put(MediaStore.MediaColumns.DISPLAY_NAME, fileName)
        put(MediaStore.MediaColumns.RELATIVE_PATH, "${Environment.DIRECTORY_DOWNLOADS}/soundlink_dump")
        put(MediaStore.MediaColumns.MIME_TYPE, "application/octet-stream")
    }
    val uri = contentResolver.insert(collection, values)!!
    return contentResolver.openOutputStream(uri, "w")!!
}
```

### 4.2 adb 拉取

```powershell
adb pull /sdcard/Download/soundlink_dump/ d:\temp\android_dump\
```

### 4.3 桌面端 dump

环境变量 `SOUNDLINK_DUMP=1` 启用，文件写到工作目录：
- `soundlink_opus.bin`（4B 长度 + 4B seq + N 字节 opus；丢包标记 length=0xFFFFFFFF）
- `soundlink_pcm_decoded.raw`（i16 LE，48kHz stereo 交错）
- `soundlink_pcm_resampled.raw`（同上，送 cpal 前）

---

## 五、分析工具

### 5.1 Python 频谱分析

```python
import numpy as np
pcm = np.fromfile('decoded.raw', dtype='<i2').astype(np.float32) / 32768.0
left = pcm[::2]  # 左声道
seg = left[:48000]  # 1 秒
fft = np.abs(np.fft.rfft(seg * np.hanning(seg.size)))
freqs = np.fft.rfftfreq(seg.size, 1/48000)
print('主频:', freqs[np.argmax(fft)], 'Hz')
ac48 = np.corrcoef(seg[:-48], seg[48:])[0, 1]
print('1kHz 自相关:', ac48)  # 应 ≈ 1.0
```

### 5.2 ffmpeg 转 wav 听一下

```
ffmpeg -f s16le -ar 48000 -ac 2 -i soundlink_pcm_decoded.raw out.wav
```

### 5.3 Opus TOC 字节分析

TOC 字节结构：`config(5bit) | stereo(1bit) | count(2bit)`
- config 30 = HYBRID mode
- config 31 = PassthroughCodec 的伪 TOC（实际是 PCM 字节被误读）

---

## 六、未完成的改进建议

1. **`default_codec()` 在生产构建中应 panic 而非 warn**：避免静默回退 PassthroughCodec
2. **直接用 libopus_sys 导出的常量**：`opusffi::OPUS_APPLICATION_AUDIO` 而非手写 `2049`
3. **Opus 解码器状态保护**：Lost 分支的 PLC 也应该有连续次数上限（已有 `PLC_CONSECUTIVE_LIMIT=8`）
4. **DebugDumper 的 IO 线程 Drop 顺序**：当前 `tx.send(Shutdown)` 后 IO 线程排空队列再退出，但 `_io_thread` 的 JoinHandle 没等待，可能进程退出时丢数据
5. **单元测试覆盖 Opus feature**：`cargo test --lib` 默认不启用 opus，`LibopusCodec` 测试被跳过 → 坑 7 的常量错误没被测试发现

---

## 七、关键文件索引

| 文件 | 作用 |
|---|---|
| `mobile/flutter_app/android/.../capture/AudioCaptureService.kt` | Android 采集 + Opus 编码 + dump |
| `mobile/flutter_app/android/.../codec/OpusEncoder.kt` | Opus 编码器 JNI 封装 |
| `mobile/flutter_app/android/.../cpp/opus_jni.c` | Opus 编码器 JNI 实现 |
| `mobile/flutter_app/lib/src/services/discovery_service.dart` | mDNS 发现（reusePort 修复） |
| `mobile/flutter_app/lib/src/services/control_client.dart` | 控制协议客户端（onDone 修复） |
| `desktop/src-tauri/src/audio/opus_codec.rs` | Opus 编解码（常量修复） |
| `desktop/src-tauri/src/audio/resampler.rs` | 漂移校正（lerp 修复） |
| `desktop/src-tauri/src/audio/output/mod.rs` | cpal 输出（owner_thread 修复） |
| `desktop/src-tauri/src/receiver.rs` | 接收引擎（Empty 修复 + DebugDumper 异步化） |
| `desktop/src-tauri/Cargo.toml` | features 依赖（tauri_app → opus） |
