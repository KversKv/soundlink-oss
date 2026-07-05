# SoundLink 项目规则（TRAE 项目级 · 优先于全局）

**元规则：本文件须 <1000 字符；新增内容优先放 `AGENTS.md`，此处只留硬红线与指针。**

开工前必读 `AGENTS.md` 与 `docs/First/`（规格以 `11-implementation-spec.md` 为准）。

## 硬红线
- 合规：禁越狱/root/私有 API；移动端仅用 ReplayKit / MediaProjection。
- 传输：音频 UDP、控制 TCP/WS；音频面必加密；第一版禁 WebRTC 等重依赖。
- 单源：协议/常量在 `shared/`，禁散落魔法值；改协议同步 04 与 11。
- 音频基线不乱改：48kHz/Stereo、Opus 10ms/128kbps、默认 Jitter 80ms。
- 移动端 Extension/Service 轻量；Rust 用 `tracing` 禁 `println!`。
- 密钥/配对码禁明文落日志；中文→简体。
- 不臆改未读代码；最小改动；不擅自新建 `*.md`；禁 `git commit`（除非明确要求）；改完跑 lint。

## 流程
- 按 `docs/First/09-roadmap.md` 分阶段推进，禁早期引入后期重依赖。
- **完成阶段任务后，必须回填 `docs/First/12-plan.md` 及对应阶段进度表。**
- Windows/PowerShell；优先用 IDE 专用工具。

细节（技术栈/结构/任务指引/合规清单）见 `AGENTS.md`。
