import type { Content } from './zh'

// English copy. Keys must exactly match zh.ts; the Content type enforces this at build time.
const REPO = 'https://github.com/KversKv/soundlink-oss'
const ASSET = (p: string) => `${import.meta.env.BASE_URL}${p}`

export const en: Content = {
  meta: {
    langName: 'English',
    otherLangName: '中文',
    otherLangHref: '../',
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
    features: 'Features',
    how: 'How it works',
    platforms: 'Platforms',
    specs: 'Specs',
    docs: 'Docs',
    guide: 'User guide',
    cta: 'Download Beta',
    langSwitch: '中文',
    logoAlt: 'SoundLink',
  },
  hero: {
    eyebrow: 'LAN audio streaming · Open source',
    title: "Send your phone's audio to your computer's headphones",
    subtitle:
      'Phone audio is encrypted and sent over the LAN to your computer, which outputs to your headphones and DAC. Pair once, auto-reconnect.',
    primaryCta: 'Download Beta',
    secondaryCta: 'View source',
    screenshotAlt: 'SoundLink desktop main window showing receiving state and pairing code',
    screenshotSrc: ASSET('assets/desktop-hero.png'),
  },
  platforms: {
    title: 'Platform support',
    note: 'Tested combinations so far: Android phone → Windows PC, and Windows → Windows. Treat other combinations as experimental.',
    status: {
      tested: 'Tested',
      ready: 'Not tested',
      planned: 'Not implemented',
    },
    items: [
      { name: 'Windows', role: 'Receiver / Sender', status: 'tested' },
      { name: 'Android', role: 'Sender', status: 'tested' },
      { name: 'macOS', role: 'Receiver (code ready)', status: 'ready' },
      { name: 'Linux', role: 'Receiver', status: 'planned' },
      { name: 'iOS', role: 'Sender (project ready)', status: 'ready' },
    ],
  },
  scenario: {
    title: 'Why it exists',
    problem:
      'You want to listen to music or movies from your phone through your desktop headphones, DAC, or speakers, but connecting a phone to them is awkward. SoundLink lets your phone stream the playing audio over the LAN to your computer, which outputs to high-quality audio devices.',
    fitsTitle: 'Good for',
    fits: ['Music', 'Long videos'],
    unfitTitle: 'Not for',
    unfit: ['Real-time gaming or voice chat (latency too high)', 'Short videos may feel slightly delayed'],
    desktopShotSrc: ASSET('assets/desktop-playing.png'),
    desktopShotAlt: 'Desktop receiving and playing audio from the phone',
    phoneShotSrc: ASSET('assets/phone-connected.png'),
    phoneShotAlt: 'Phone broadcasting audio to the computer',
  },
  differentiators: {
    title: 'Three differentiators',
    items: [
      {
        title: 'No developer mode',
        body: 'Unlike sndcpy / scrcpy, no USB debugging required. Connect both devices to the same LAN and enter a pairing code.',
      },
      {
        title: 'Encrypted by default',
        body: 'Audio is encrypted end to end with ChaCha20-Poly1305. Keys are negotiated via X25519 and stored in the OS keyring, never in plaintext. Zero telemetry.',
      },
      {
        title: 'Truly free and open source',
        body: 'MIT licensed. No subscriptions, no ads, no feature gates. Fully auditable code, contributions welcome.',
      },
    ],
  },
  how: {
    title: 'How it works',
    steps: [
      {
        verb: 'Start receiving',
        body: 'Launch SoundLink on your computer, pick an output device, switch to receiver role. An 8-digit pairing code appears.',
        imgSrc: ASSET('assets/desktop-pairing.png'),
        imgAlt: 'Desktop showing an 8-digit pairing code',
      },
      {
        verb: 'Enter the code',
        body: 'On your phone, select the computer from the device list (auto-discovered via mDNS) and enter the pairing code.',
        imgSrc: ASSET('assets/phone-pairing.png'),
        imgAlt: 'Phone entering the pairing code',
      },
      {
        verb: 'Start playing',
        body: 'The phone starts capturing and playing audio; sound comes out of the computer instantly. Pairing persists and auto-reconnects.',
        imgSrc: ASSET('assets/desktop-playing.png'),
        imgAlt: 'Desktop showing device connected and playing',
      },
    ],
    guideLink: 'Read the full user guide →',
  },
  specs: {
    title: 'Technical specs',
    groups: [
      {
        name: 'Audio',
        rows: [
          ['Sample rate', '48 kHz'],
          ['Channels', 'Stereo'],
          ['Codec', 'Opus 10 ms'],
          ['Bitrate', '128 kbps'],
          ['Jitter buffer', '80 ms (default)'],
        ],
      },
      {
        name: 'Security',
        rows: [
          ['Key exchange', 'X25519'],
          ['Signatures', 'Ed25519'],
          ['Encryption', 'ChaCha20-Poly1305'],
          ['Key storage', 'OS keyring'],
          ['Telemetry', 'None'],
        ],
      },
      {
        name: 'Transport',
        rows: [
          ['Audio', 'UDP'],
          ['Control', 'TCP'],
          ['Scope', 'LAN only'],
          ['Latency target', '~100 ms class'],
        ],
      },
    ],
  },
  limitations: {
    title: 'Known limitations',
    intro: 'Honesty is a feature. These limits come from the current architecture and platform policies.',
    groups: [
      {
        name: 'Network & scope',
        items: [
          'LAN only; no internet or NAT traversal. AP isolation or guest networks will block it.',
          'Single receiver: one sender pairs with one receiver at a time.',
          'No USB mode; data never goes over a cable.',
        ],
      },
      {
        name: 'Content & experience',
        items: [
          'DRM-protected content cannot be captured; some streaming audio goes silent. SoundLink does not and will not bypass this.',
          'Latency targets music and video; not suitable for gaming or voice chat.',
          'Desktop UI is Chinese-only for now; English i18n is planned.',
          'Installers are not code-signed; Windows SmartScreen will warn on first run.',
        ],
      },
    ],
  },
  download: {
    title: 'Download Beta',
    body: 'Get the latest pre-release installers from GitHub Releases.',
    primaryCta: 'Download Beta',
    secondaryCta: 'View source',
    shaTitle: 'Verify your download',
    shaBody: 'Each release lists the SHA256 of every installer. After downloading, compare it in PowerShell:',
    shaCmd: 'Get-FileHash .\\SoundLink-Setup.exe -Algorithm SHA256',
    smartScreenTitle: 'About the SmartScreen warning',
    smartScreenBody:
      'Installers are not code-signed, so Windows SmartScreen shows an "unknown publisher" warning on first run. This is expected; verify the SHA256 above before running.',
    testedTitle: 'Tested scope',
    testedBody: 'Only Android → Windows and Windows → Windows have been tested so far. Treat other combinations as experimental.',
  },
  guide: {
    eyebrow: 'User guide',
    title: 'From install to sound, end to end',
    intro:
      'This guide covers: preparation, phone → PC, PC → PC, settings, and FAQ. Every step uses real screenshots — just follow along.',
    backHome: 'Back to home',
    homeHref: '../',
    otherLangHref: '../../guide/',
    toc: [
      { id: 'prepare', label: 'Preparation' },
      { id: 'phone-to-pc', label: 'Phone → PC' },
      { id: 'pc-to-pc', label: 'PC → PC' },
      { id: 'settings', label: 'Settings' },
      { id: 'faq', label: 'FAQ' },
    ],
    prepare: {
      title: 'Preparation',
      items: [
        {
          title: 'Same LAN',
          body: 'Connect the phone and the computer to the same Wi-Fi / subnet. Disable AP isolation on the router and avoid guest networks, otherwise the devices cannot see each other.',
        },
        {
          title: 'Install SoundLink',
          body: 'Get the desktop and Android installers from GitHub Releases. Launch after install — no account, no sign-in.',
        },
        {
          title: 'Allow the firewall',
          body: 'On first launch, allow "Private networks" in the Windows firewall prompt. Control uses TCP 47810, audio uses UDP 47811.',
        },
      ],
    },
    phoneToPc: {
      title: 'Phone → PC',
      note: 'The most common path: the phone captures audio, the PC receives it and outputs to your headphones or DAC.',
      steps: [
        {
          title: 'Start receiving on the PC',
          body: 'Open SoundLink and switch to Receiver mode. An 8-digit pairing code and the local address appear. Pick the output device under Settings → Audio.',
          imgSrc: ASSET('assets/desktop-pairing.png'),
          imgAlt: 'Desktop receiver mode showing the 8-digit pairing code',
          kind: 'desktop',
        },
        {
          title: 'Pick the device and enter the code',
          body: 'The app auto-discovers computers on the LAN via mDNS. If nothing shows up, tap "Manual IP" and enter the IP from the PC\'s local address. Select the device, then enter the 8-digit code.',
          imgSrc: ASSET('assets/phone-pairing.png'),
          imgAlt: 'Phone selecting a device and entering the pairing code',
          kind: 'phone',
        },
        {
          title: 'Grant screen capture',
          body: 'On first broadcast, Android shows a MediaProjection consent dialog — tap "Start now". This is the official capture API; DRM-protected content cannot be captured.',
          imgSrc: ASSET('assets/phone-consent.png'),
          imgAlt: 'Phone MediaProjection consent dialog',
          kind: 'phone',
        },
        {
          title: 'Start playing',
          body: 'The phone shows "Broadcasting" with a persistent capture notification; sound comes out of the PC instantly. Pairing persists and reconnects automatically next time.',
          imgSrc: ASSET('assets/phone-connected.png'),
          imgAlt: 'Phone broadcasting audio to the computer',
          kind: 'phone',
        },
      ],
    },
    pcToPc: {
      title: 'PC → PC',
      body: 'To stream between two computers: switch the sending PC to Sender mode and pick a capture source (system loopback or the 440Hz sine test source). "Discovered receivers" lists receivers on the LAN; if none appear, click "Scan LAN" or enter the receiver\'s address manually (e.g. 192.168.1.100:47810), then enter its pairing code to start.',
      imgSrc: ASSET('assets/desktop-sender.png'),
      imgAlt: 'Desktop sender mode: capture source and receiver selection',
      points: [
        'The receiving PC must be in Receiver mode showing its pairing code',
        'Capture sources can be switched anytime without re-pairing',
        'Windows → Windows is a tested combination',
      ],
    },
    settings: {
      title: 'Settings',
      imgSrc: ASSET('assets/desktop-settings.png'),
      imgAlt: 'Desktop settings: startup, close behavior, device and audio',
      points: [
        ['Launch at startup', 'Combined with "auto-start receiving", your PC can stay a permanent receiver — connect from the phone anytime.'],
        ['Close-window behavior', 'Choose ask every time, minimize to tray, or quit. Tray mode keeps it out of the way.'],
        ['Device name (mDNS broadcast)', 'The name shown on the LAN. Rename it when several devices coexist.'],
        ['Audio', 'Receiver mode picks the output device (headphones / DAC); sender mode picks the default capture source. Jitter buffer defaults to 80ms — raise it on lossy networks.'],
      ],
    },
    faq: {
      title: 'FAQ',
      items: [
        {
          q: 'The phone can\'t find the computer?',
          a: 'Make sure both devices are on the same subnet and AP isolation is off. If mDNS is filtered, use "Manual IP" with the address shown on the PC, and check the Windows firewall rules.',
        },
        {
          q: 'Pairing code rejected or locked out?',
          a: 'Temporary codes are valid for 120 seconds — refresh on the PC when expired. Five wrong attempts trigger a 60-second lockout. Long-term codes are reusable but still protected by the lockout.',
        },
        {
          q: 'Connected, but no sound?',
          a: 'Check the selected output device on the PC; make sure the content is not DRM-protected (some streaming apps go silent); confirm the phone notification shows an active capture.',
        },
        {
          q: 'Noticeable latency?',
          a: 'The default 80ms jitter buffer is normal for music and long videos. Gaming and voice chat are out of scope — see "Known limitations" on the home page.',
        },
        {
          q: 'Windows SmartScreen warning?',
          a: 'Installers are not code-signed, so the warning is expected. Verify the SHA256 from the release page matches your download, then choose "Run anyway".',
        },
        {
          q: 'How do I stop broadcasting?',
          a: 'Tap "Stop broadcasting" on the phone or stop from the notification; click "Stop receiving" on the PC. For daily use, set close-window behavior to minimize to tray.',
        },
      ],
    },
    cta: {
      title: 'Ready?',
      body: 'Download the latest Beta from GitHub Releases and finish your first pairing with the steps above.',
      primary: 'Download Beta',
      secondary: 'Back to home',
    },
  },
  footer: {
    tagline: 'LAN audio streaming, free and open source.',
    columns: [
      {
        name: 'Project',
        links: [
          { label: 'GitHub repo', href: REPO },
          { label: 'Download Beta', href: `${REPO}/releases/latest` },
          { label: 'CHANGELOG', href: `${REPO}/blob/main/CHANGELOG.md` },
        ],
      },
      {
        name: 'Docs',
        links: [
          { label: 'User docs', href: `${REPO}/tree/main/docs/user` },
          { label: 'Privacy policy', href: `${REPO}/blob/main/docs/privacy.md` },
          { label: 'MIT license', href: `${REPO}/blob/main/LICENSE` },
        ],
      },
      {
        name: 'Community',
        links: [
          { label: 'Issues', href: `${REPO}/issues` },
          { label: 'Discussions', href: `${REPO}/discussions` },
          { label: 'Security reports', href: `${REPO}/blob/main/SECURITY.md` },
        ],
      },
    ],
    copyright: 'MIT License · Copyright (c) 2026 KversKv',
  },
}
