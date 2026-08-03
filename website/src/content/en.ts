import type { Content } from './zh'

// English copy. Keys must exactly match zh.ts; the Content type enforces this at build time.
const REPO = 'https://github.com/KversKv/SoundLink'
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
  },
  nav: {
    features: 'Features',
    how: 'How it works',
    platforms: 'Platforms',
    specs: 'Specs',
    docs: 'Docs',
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
    screenshotSrc: ASSET('assets/placeholder.svg'),
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
    bgAlt: 'A pair of headphones and a phone on a desk',
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
        imgSrc: ASSET('assets/placeholder.svg'),
        imgAlt: 'Desktop showing an 8-digit pairing code',
      },
      {
        verb: 'Enter the code',
        body: 'On your phone, select the computer from the device list (auto-discovered via mDNS) and enter the pairing code.',
        imgSrc: ASSET('assets/placeholder.svg'),
        imgAlt: 'Phone entering the pairing code',
      },
      {
        verb: 'Start playing',
        body: 'The phone starts capturing and playing audio; sound comes out of the computer instantly. Pairing persists and auto-reconnects.',
        imgSrc: ASSET('assets/placeholder.svg'),
        imgAlt: 'Desktop showing device connected and playing',
      },
    ],
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
