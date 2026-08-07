// 中文文案（默认语言）。键名与 en.ts 完全一致，由类型检查保证。
// 单源约束：平台矩阵 / 已知限制 / 音频基线 / 加密算法以 README.md 与
// docs/First/11-implementation-spec.md 为准，修改数字前必须先核对单源。

const REPO = 'https://github.com/KversKv/soundlink-oss'
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
    guide: './guide/',
  },
  nav: {
    features: '特性',
    how: '原理',
    platforms: '平台',
    specs: '规格',
    docs: '文档',
    guide: '使用指南',
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
    screenshotSrc: ASSET('assets/desktop-hero.png'),
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
    desktopShotSrc: ASSET('assets/desktop-playing.png'),
    desktopShotAlt: '桌面端正在接收并播放来自手机的音频',
    phoneShotSrc: ASSET('assets/phone-connected.png'),
    phoneShotAlt: '手机端正在向电脑广播音频',
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
        imgSrc: ASSET('assets/desktop-pairing.png'),
        imgAlt: '桌面端显示 8 位配对码',
      },
      {
        verb: '输入配对码',
        body: '手机端在设备列表选中电脑（mDNS 自动发现），输入配对码完成配对。',
        imgSrc: ASSET('assets/phone-pairing.png'),
        imgAlt: '手机端输入配对码',
      },
      {
        verb: '开始播放',
        body: '手机开始采集并播放音频，电脑端即刻出声。配对信息持久化，下次自动重连。',
        imgSrc: ASSET('assets/desktop-playing.png'),
        imgAlt: '桌面端显示设备已连接、播放中',
      },
    ],
    guideLink: '查看详细使用指南 →',
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
  guide: {
    eyebrow: '使用指南',
    title: '从安装到出声，一次讲清',
    intro:
      '本指南覆盖：准备工作、手机 → 电脑、电脑 → 电脑、设置说明与常见问题。全程真实截图，跟着做即可。',
    backHome: '返回首页',
    homeHref: '../',
    otherLangHref: '../en/guide/',
    toc: [
      { id: 'prepare', label: '准备工作' },
      { id: 'phone-to-pc', label: '手机 → 电脑' },
      { id: 'pc-to-pc', label: '电脑 → 电脑' },
      { id: 'settings', label: '设置说明' },
      { id: 'faq', label: '常见问题' },
    ] as { id: string; label: string }[],
    prepare: {
      title: '准备工作',
      items: [
        {
          title: '同一局域网',
          body: '手机与电脑连接同一个 Wi-Fi / 网段。关闭路由器的 AP 隔离，不要使用访客网络，否则设备互相不可见。',
        },
        {
          title: '安装 SoundLink',
          body: '电脑端与 Android 端安装包均从 GitHub Releases 获取。安装后启动即可，无需注册、无需登录。',
        },
        {
          title: '放行防火墙',
          body: '首次启动时，Windows 防火墙弹窗请勾选「专用网络」并允许。控制通道走 TCP 47810，音频走 UDP 47811。',
        },
      ] as { title: string; body: string }[],
    },
    phoneToPc: {
      title: '手机 → 电脑',
      note: '最常用路径：手机采集音频，电脑接收并输出到耳机或声卡。',
      steps: [
        {
          title: '电脑端开启接收',
          body: '打开 SoundLink，切到「接收模式」，界面显示 8 位配对码与本机地址。输出设备可在「设置 → 音频」中指定。',
          imgSrc: ASSET('assets/desktop-pairing.png'),
          imgAlt: '桌面端接收模式，显示 8 位配对码',
          kind: 'desktop',
        },
        {
          title: '手机端选设备、输配对码',
          body: 'App 通过 mDNS 自动发现局域网内的电脑；搜不到时点「手动 IP」，输入电脑端「本机地址」中的 IP。选中设备后输入 8 位配对码。',
          imgSrc: ASSET('assets/phone-pairing.png'),
          imgAlt: '手机端选择设备并输入配对码',
          kind: 'phone',
        },
        {
          title: '授权屏幕采集',
          body: '首次广播时系统弹出 MediaProjection 授权窗口，点击「立即开始」。这是 Android 官方采集能力；受 DRM 保护的内容无法采集。',
          imgSrc: ASSET('assets/phone-consent.png'),
          imgAlt: '手机端 MediaProjection 授权弹窗',
          kind: 'phone',
        },
        {
          title: '开始播放',
          body: '手机提示「正在广播」，通知栏显示采集状态；电脑端即刻出声。配对信息会保存，下次打开自动重连。',
          imgSrc: ASSET('assets/phone-connected.png'),
          imgAlt: '手机端正在广播音频到电脑',
          kind: 'phone',
        },
      ] as { title: string; body: string; imgSrc: string; imgAlt: string; kind: 'desktop' | 'phone' }[],
    },
    pcToPc: {
      title: '电脑 → 电脑',
      body: '两台电脑互传：发送端切到「发送模式」，在「采集源」中选择要发送的音频（系统环回或 440Hz 正弦测试源）。「发现 Receiver」会列出局域网内的接收端；未发现时点「扫描局域网」，或手动输入接收端的本机地址（如 192.168.1.100:47810），再输入配对码即可开始发送。',
      imgSrc: ASSET('assets/desktop-sender.png'),
      imgAlt: '桌面端发送模式，选择采集源与接收端',
      points: [
        '接收端需处于「接收模式」并显示配对码',
        '采集源可随时切换，无需重新配对',
        'Windows → Windows 组合已实测通过',
      ] as string[],
    },
    settings: {
      title: '设置说明',
      imgSrc: ASSET('assets/desktop-settings.png'),
      imgAlt: '桌面端设置页：启动、关闭行为、设备与音频',
      points: [
        ['开机自启动', '配合「自启动后自动开启接收」，可把电脑常驻为接收端，手机随连随用。'],
        ['关闭窗口行为', '可选每次询问、最小化到托盘或直接退出；常驻托盘不打扰日常使用。'],
        ['设备名（mDNS 广播名）', '局域网内显示的名字，多台设备并存时建议改名区分。'],
        ['音频', '接收模式选输出设备（耳机 / 声卡），发送模式选默认采集源；Jitter 缓冲默认 80ms，网络抖动大时可适当调高。'],
      ] as [string, string][],
    },
    faq: {
      title: '常见问题',
      items: [
        {
          q: '手机搜不到电脑？',
          a: '确认两端在同一网段、路由器未开启 AP 隔离；mDNS 被过滤时使用「手动 IP」，地址见电脑端「本机地址」；同时检查 Windows 防火墙是否放行。',
        },
        {
          q: '配对码报错或被锁定？',
          a: '临时配对码 120 秒内有效，过期在电脑端点「刷新」；连续错误 5 次触发 60 秒锁定，稍候重试。长期配对码可重复使用，同样受错误锁定保护。',
        },
        {
          q: '连上了但没有声音？',
          a: '检查电脑端输出设备是否选对；确认播放内容未受 DRM 保护（部分流媒体会静音）；确认手机通知栏显示正在采集。',
        },
        {
          q: '感觉有延迟？',
          a: '默认 Jitter 缓冲 80ms，听音乐、看长视频属正常体验；游戏与连麦不在适用范围，详见首页「已知限制」。',
        },
        {
          q: 'Windows 弹出 SmartScreen 告警？',
          a: '安装包未购买代码签名证书，告警属预期行为。核对 Release 页的 SHA256 与下载文件一致后，选择「仍要运行」即可。',
        },
        {
          q: '如何停止广播？',
          a: '手机端点「停止广播」，或在通知栏停止采集；电脑端点「停止接收」。日常可将关闭窗口行为设为最小化到托盘。',
        },
      ] as { q: string; a: string }[],
    },
    cta: {
      title: '准备好了？',
      body: '从 GitHub Releases 下载最新 Beta，按上面的步骤完成第一次配对。',
      primary: '下载 Beta',
      secondary: '返回首页',
    },
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
