# Debug Session: audio-params-probe

Status: [CLOSED] — 结论与实现清单见 [FT-0012](../FunctionTasks/0012-2026-07-07-audio-params-probe-fix.md)

## Hypotheses

1. Desktop audio参数没有明显效果，是因为 UI/配置只保存参数，发送端 Opus 编码器、发送循环帧长和 UDP packet header 仍使用固定 48kHz/Stereo/10ms/128kbps 常量。
2. 手机端自动探测没有明显生效，是因为当前实现只是按当前 jitter 档位切换码率/帧长，没有执行网络探测、没有暂停音频流，也没有把推荐结果显式反馈给用户。
3. 手机日志中的 gralloc4 ERROR 与 Flutter Impeller/Vulkan 图形后端相关，和音频采集/自动探测没有直接因果关系。
4. Android 原生采集虽然 encoder/header 读取 SessionConfig，但 AudioRecord 循环仍硬编码 48kHz/Stereo/10ms，导致非默认参数与实际 PCM 帧不一致。
5. 桌面 Receiver 对非默认采样率/声道/帧长的解码/播放链路仍未动态重建，因此这些参数不能承诺运行时立即生效。

## Evidence

- 用户提供的 Flutter 日志显示 App 成功启动、原生 opus 库加载成功、AudioRecord 使用 REMOTE_SUBMIX，未出现自动探测异常栈。
- 日志中的 `E/gralloc4 ... unsupported format 0x3b` 出现在 Flutter UI 渲染上下文，且伴随 Impeller/Vulkan 启动信息。

## Plan

- 补齐桌面发送端实际 bitrate 与 packet header 应用，避免“只打印 Jitter”。
- 对不完整支持的 sample_rate/channels/frame_duration 做明确限制与 UI 提示，避免配置和实际链路不一致。
- 手机自动探测改为暂停当前音频流、执行 TCP 连接延迟探测、应用推荐并返回可展示结果。
- 手机设置页自动探测后显示弹窗。
- 关闭 Android debug 下 Impeller，消除 gralloc4 图形后端 ERROR。
