# AGENTS.md · SoundLink

> 面向 TRAE / AI 协作代理的工作说明。开始任何任务前先读本文件与 `.trae/rules/project-rules.md`。

## 项目一句话

SoundLink：面向头戴式耳机用户的**局域网音频流转**软件。手机（iOS/Android）音频 → 局域网 → 电脑音频设备；支持电脑到电脑互传。

## 权威信息来源

- 顶层导航：`docs/First/SoundLinkStructrue.md`
- 专题文档：`docs/First/01~10-*.md`
- **实现规格（编码依据）：`docs/First/11-implementation-spec.md`**
- 工程规则：`.trae/rules/project-rules.md`

> 设计决策以 `docs/First/` 为准。若代码与文档冲突，先确认哪个是最新意图，再同步另一方，**不要单方面偏离**。

## 仓库结构（速览）

- `mobile/ios`、`mobile/android`：移动发送端
- `desktop/src-tauri`（Rust 核心）、`desktop/ui`（前端）：桌面端
- `shared/`：跨端协议与常量
- `docs/`：设计文档

详见 `docs/First/10-project-structure.md`。

## 技术栈

- iOS：Swift + SwiftUI + ReplayKit
- Android：Kotlin + Compose + MediaProjection
- 桌面：Tauri 2 + Rust（tokio）+ React/TS
- 编解码：libopus；传输：UDP(音频)+TCP/WS(控制)；加密：ChaCha20-Poly1305

## 代理工作准则

1. **先读后写**：修改任何文件前先阅读它；不臆改未读代码。
2. **最小改动**：只做被要求或明确必要的改动，不顺手重构/加特性。
3. **对齐文档**：涉及架构/协议/延迟等决策时，引用 `docs/First/` 对应文档。
4. **合规红线**：不引入越狱/root/私有 API；移动端采集仅用 ReplayKit / MediaProjection 官方能力。
5. **阶段意识**：按 `docs/First/09-roadmap.md` 的阶段推进，不在早期阶段引入后期重依赖（如 WebRTC）。
6. **不擅自创建文档**：除非用户明确要求，不新增 README/文档文件。
7. **命令与工具**：Windows/PowerShell 环境；优先使用 IDE 提供的专用工具而非 shell。

## 当前状态

- 阶段：规划完成，仓库为**骨架 + 占位**。
- 尚未进行脚手架初始化（Tauri/Xcode/Gradle）与可运行实现。

## 常见任务指引

| 任务 | 起点 |
|---|---|
| 改协议 | `shared/protocol` + `docs/First/04-protocol.md` |
| 桌面接收/输出 | `desktop/src-tauri/src/{network,audio}` |
| iOS 采集 | `mobile/ios/BroadcastExtension` |
| Android 采集 | `mobile/android/.../capture` |
| 配对/安全 | 各端 `pairing` + `docs/First/05-pairing-security.md` |

## 触发式必读（按任务查文档）

架构→02；音频链路→03；协议→04；配对安全→05；延迟→06；选型→07；平台合规→08；阶段→09；目录→10；**编码规格→11**；**计划/进度→12**。

## 平台合规检查项

- iOS：仅 ReplayKit；引导用户手动开启广播；标注 DRM 不可采。
- Android：MediaProjection + 前台 Service + 通知；标注部分应用不可采。
- 产品文案明确：不保证所有应用/受保护内容可用。

## 进度回填约束

完成任一阶段任务后，**必须回填** `docs/First/12-plan.md` 总表状态与对应阶段进度表（勾选任务、填写完成日期/备注）。规则详见该文件。
