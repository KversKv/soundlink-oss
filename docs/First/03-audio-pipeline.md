# 03 · 音频链路（Audio Pipeline）

## 1. 发送端链路

### iOS
```text
ReplayKit CMSampleBuffer (.audioApp)
  → 提取 AudioBufferList
  → PCM 归一化: 48kHz / Stereo / Int16(或Float32)
  → Opus 编码 (10ms 帧, 128kbps 起步)
  → Packetize
  → Encrypt (ChaCha20-Poly1305)
  → UDP Send
```

### Android
```text
AudioPlaybackCapture (MediaProjection)
  → AudioRecord 读取 PCM
  → PCM 归一化: 48kHz / Stereo / Int16
  → Opus 编码 (10ms 帧)
  → Packetize
  → Encrypt
  → UDP Send
```

### 桌面 Sender（后续）
```text
WASAPI Loopback / ScreenCaptureKit
  → PCM 48kHz Stereo
  → Opus 编码
  → Packetize → Encrypt → UDP Send
```

## 2. 接收端链路（桌面）

```text
UDP Receive
  → Decrypt + Auth 校验
  → 按 sequence 重排 (丢弃过期包)
  → Jitter Buffer
  → Opus Decode (含 PLC 丢包补偿)
  → Resample / 时钟漂移校正
  → Audio Device Output (WASAPI/CoreAudio/PipeWire)
```

## 3. 音频参数

| 参数 | 推荐值 | 说明 |
|---|---|---|
| 采样率 | 48 kHz | 全链路统一 |
| 声道 | Stereo | |
| 位深 | Int16 / Float32 | 采集端归一化 |
| 编码 | Opus | |
| Opus 帧长 | 10 ms（后续 5 ms） | 稳定性优先 |
| Opus 码率 | 128 kbps 起步，自适应 | |
| 每包时长 | 10 ms | 与帧长一致 |
| 传输 | UDP 单播 | |

## 4. Jitter Buffer 策略

| 模式 | 缓冲 | 适用 |
|---|---:|---|
| 低延迟 | 40 ms | 网络稳定 |
| 平衡（默认） | 80 ms | 默认 |
| 稳定 | 150 ms | Wi-Fi 较差 |

- 第一版默认**平衡模式 80 ms**。
- 后续做**自适应 Jitter Buffer**：根据丢包率/抖动动态调整。

## 5. 时钟漂移与重采样

- 发送端与接收端时钟不完全同步，长时间播放会积累漂移（缓冲耗尽或溢出）。
- 策略：监测缓冲水位，微调重采样比率（软重采样）平滑吸收漂移，避免爆音/断音。

## 6. 丢包与弱网处理

- UDP 丢包：过期包直接丢弃，不重传。
- Opus PLC：解码端对丢失帧做补偿。
- 码率自适应：根据丢包统计动态调整 Opus 码率。
- 统计上报：丢包率、抖动、缓冲水位、估算延迟 → 控制面上报，用于 UI 展示与调参。
