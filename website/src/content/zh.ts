// 中文文案（默认语言）。键名与 en.ts 完全一致，由类型检查保证。
// 单源约束：平台矩阵 / 已知限制 / 音频基线 / 加密算法以 README.md 与
// docs/First/11-implementation-spec.md 为准，修改数字前必须先核对单源。

const REPO = 'https://github.com/KversKv/SoundLink'
const ASSET = (p: string) => `${import.meta.env.BASE_URL}${p}`

export const zh = {
  meta: {
    langName: '中文',
    otherLangName: 'English',
    otherLangHref: './en/',
  },
  links: {
    repo: REPO,
    releases: `${REPO}/releases/latest`,
    issues: `${REPO}/issues`,
    discussions: `${REPO}/discussions`,
    security: `${REPO}/blob/main/SECURITY.md`,
    license: `${REPO}/blob/main/LICENSE`,
    privacy: `${REPO}/blob/main/docs/privacy.md`,
    changelog: `${REPO}/blob/main/CHANGELOG.md`,
    docs: `${REPO}/tree/main/docs/user`,
  },
  nav: {
    features: '特性',
    how: '原理',
    platforms: '平台',
    specs: '规格',
    docs: '文档',
    cta: '下载 Beta',
    langSwitch: 'EN',
    logoAlt: 'SoundLink',
  },
  hero: {
    eyebrow: '局域网音频流转 · 开源免费',
    title: '把手机的声音，送到电脑的耳机上',
    subtitle: '手机音频经局域网加密发送到电脑，由电脑输出到你的耳机与声卡。一次配对，自动重连。',
    primaryCta: '下载 Beta',
    secondaryCta: '查看源码',
    screenshotAlt: 'SoundLink 桌面端主界面，显示接收状态与配对码',
    // TODO(A1): 替换为真实桌面端截图 ≥2400×1500 PNG/WebP
    screenshotSrc: ASSET('assets/placeholder.svg'),
  },
  platforms: {
    title: '平台支持',
    note: '当前实测通过的组合：Android 手机 → Windows 电脑、Windows → Windows。其他组合请勿按「可用」预期。',
    status: {
      tested: '实测可用',
      ready: '未实测',
      planned: '未实装',
    },
    items: [
      { name: 'Windows', role: '接收端 / 发送端', status: 'tested' },
      { name: 'Android', role: '发送端', status: 'tested' },
      { name: 'macOS', role: '接收端（代码就绪）', status: 'ready' },
      { name: 'Linux', role: '接收端', status: 'planned' },
      { name: 'iOS', role: '发送端（工程就绪）', status: 'ready' },
    ] as { name: string; role: string; status: 'tested' | 'ready' | 'planned' }[],
  },
  scenario: {
    title: '为什么需要它',
    problem:
      '手机上的音乐、电影想用桌面的耳机、声卡、音箱听，但手机直连这些设备并不方便。SoundLink 让手机把正在播放的音频通过局域网发给电脑，由电脑输出到高品质音频设备。',
    fitsTitle: '适用',
    fits: ['听音乐', '看长视频'],
    unfitTitle: '不适用',
    unfit: ['游戏、连麦等实时互动（延迟不满足）', '短视频可能感知轻微延迟'],
    // TODO(A4): 替换为真实场景图（耳机 + 桌面 + 手机）横向 ≥2400px
    bgAlt: '桌面上的一副耳机与一部手机',
  },
  differentiators: {
    title: '三条差异化',
    items: [
      {
        title: '免开发者模式',
        body: '不像 sndcpy / scrcpy 需要打开 USB 调试。手机与电脑连同一局域网，输入配对码即可使用。',
      },
      {
        title: '默认加密',
        body: '音频面全程 ChaCha20-Poly1305 加密，密钥经 X25519 协商并写入系统钥匙串，不落明文。零遥测、零上报。',
      },
      {
        title: '真开源免费',
        body: 'MIT 许可证，无订阅、无广告、无功能墙。代码完全公开，欢迎审计与贡献。',
      },
    ],
  },
  how: {
    title: '工作原理',
    steps: [
      {
        verb: '开启接收',
        body: '电脑端启动 SoundLink，选择输出设备并切到接收角色，界面显示 8 位配对码。',
        // TODO(A1): 桌面端开启接收、显示配对码截图
        imgSrc: ASSET('assets/placeholder.svg'),
        imgAlt: '桌面端显示 8 位配对码',
      },
      {
        verb: '输入配对码',
        body: '手机端在设备列表选中电脑（mDNS 自动发现），输入配对码完成配对。',
        // TODO(A2): 手机端配对界面截图
        imgSrc: ASSET('assets/placeholder.svg'),
        imgAlt: '手机端输入配对码',
      },
      {
        verb: '开始播放',
        body: '手机开始采集并播放音频，电脑端即刻出声。配对信息持久化，下次自动重连。',
        // TODO(A3): 桌面端设备已连接 / 播放中截图
        imgSrc: ASSET('assets/placeholder.svg'),
        imgAlt: '桌面端显示设备已连接、播放中',
      },
    ],
  },
  specs: {
    title: '技术规格',
    groups: [
      {
        name: '音频',
        rows: [
          ['采样率', '48 kHz'],
          ['声道', 'Stereo'],
          ['编码', 'Opus 10 ms'],
          ['码率', '128 kbps'],
          ['Jitter 缓冲', '80 ms（默认）'],
        ],
      },
      {
        name: '安全',
        rows: [
          ['密钥协商', 'X25519'],
          ['签名', 'Ed25519'],
          ['加密', 'ChaCha20-Poly1305'],
          ['密钥存储', 'OS keyring'],
          ['遥测', '零遥测'],
        ],
      },
      {
        name: '传输',
        rows: [
          ['音频', 'UDP'],
          ['控制', 'TCP'],
          ['范围', '仅局域网'],
          ['延迟目标', '100 ms 级'],
        ],
      },
    ] as { name: string; rows: [string, string][] }[],
  },
  limitations: {
    title: '已知限制',
    intro: '诚实即卖点。以下限制来自当前架构与平台策略，使用前请知悉。',
    groups: [
      {
        name: '网络与范围',
        items: [
          '仅局域网，不支持公网与 NAT 穿透；AP 隔离或访客网络会阻断。',
          '单接收端：当前一个发送端对应一个接收端。',
          '无 USB 模式，不走数据线。',
        ],
      },
      {
        name: '内容与体验',
        items: [
          'DRM 内容不可采，部分流媒体音频会静音，SoundLink 无法也不试图绕过。',
          '延迟面向听音乐与看视频，不适合游戏与连麦。',
          '桌面 UI 仅中文，英文 i18n 在规划中。',
          '安装包未代码签名，Windows SmartScreen 首次运行会告警。',
        ],
      },
    ],
  },
  download: {
    title: '下载 Beta',
    body: '从 GitHub Releases 获取最新的 Pre-release 安装包。',
    primaryCta: '下载 Beta',
    secondaryCta: '查看源码',
    shaTitle: '校验安装包',
    shaBody: 'Release 页提供每个安装包的 SHA256。下载后可在 PowerShell 中执行以下命令比对：',
    shaCmd: 'Get-FileHash .\\SoundLink-Setup.exe -Algorithm SHA256',
    smartScreenTitle: '关于 SmartScreen 告警',
    smartScreenBody:
      '安装包未购买代码签名证书，Windows SmartScreen 首次运行会提示「未知发布者」。这是预期行为，可先用上面的 SHA256 校验后再运行。',
    testedTitle: '实测范围',
    testedBody: '当前仅实测 Android → Windows 与 Windows → Windows 两种组合，其他组合请按实验性预期。',
  },
  footer: {
    tagline: '局域网音频流转，开源免费。',
    columns: [
      {
        name: '项目',
        links: [
          { label: 'GitHub 仓库', href: REPO },
          { label: '下载 Beta', href: `${REPO}/releases/latest` },
          { label: 'CHANGELOG', href: `${REPO}/blob/main/CHANGELOG.md` },
        ],
      },
      {
        name: '文档',
        links: [
          { label: '用户文档', href: `${REPO}/tree/main/docs/user` },
          { label: '隐私政策', href: `${REPO}/blob/main/docs/privacy.md` },
          { label: 'MIT 许可证', href: `${REPO}/blob/main/LICENSE` },
        ],
      },
      {
        name: '社区',
        links: [
          { label: 'Issues', href: `${REPO}/issues` },
          { label: 'Discussions', href: `${REPO}/discussions` },
          { label: '安全上报', href: `${REPO}/blob/main/SECURITY.md` },
        ],
      },
    ],
    copyright: 'MIT License · Copyright (c) 2026 KversKv',
  },
}

export type Content = typeof zh
