import { ArrowLeft, Plus, Check } from '@phosphor-icons/react'
import { Button } from './components/Button'
import { SectionShell } from './components/SectionShell'
import { Footer } from './sections/Footer'
import type { Content } from './content/zh'

// 使用指南页：精简顶栏 + 目录锚点 + 分节图文步骤
function GuideNav({ c }: { c: Content }) {
  const g = c.guide
  return (
    <header className="sticky top-0 z-50 border-b border-border bg-bg/85 backdrop-blur">
      <div className="mx-auto flex h-[64px] w-full max-w-6xl items-center justify-between px-6">
        <a
          href={g.homeHref}
          className="flex items-center gap-2 text-sm text-text-dim hover:text-text"
        >
          <ArrowLeft size={16} weight="bold" />
          <span>{g.backHome}</span>
        </a>
        <span className="hidden items-center gap-2 font-semibold tracking-tight sm:flex">
          <img src={`${import.meta.env.BASE_URL}favicon.svg`} alt="" className="h-6 w-6" />
          <span>SoundLink</span>
        </span>
        <div className="flex items-center gap-3">
          <a
            href={g.otherLangHref}
            className="rounded-[8px] border border-border px-3 py-1.5 text-sm text-text-dim hover:text-text"
          >
            {c.nav.langSwitch}
          </a>
          <a
            href={c.links.releases}
            target="_blank"
            rel="noreferrer"
            className="btn-press rounded-[8px] bg-accent px-4 py-1.5 text-sm font-medium text-[#06231a] hover:bg-[#3ce8b6]"
          >
            {c.nav.cta}
          </a>
        </div>
      </div>
    </header>
  )
}

export function GuideApp({ c }: { c: Content }) {
  const g = c.guide
  return (
    <>
      <GuideNav c={c} />
      <main>
        {/* 页头 + 目录 */}
        <section className="border-b border-border">
          <div className="mx-auto w-full max-w-6xl px-6 py-16 md:py-24">
            <p className="anim-fade-up text-xs font-medium uppercase tracking-[0.18em] text-accent">
              {g.eyebrow}
            </p>
            <h1 className="anim-fade-up mt-4 max-w-[20ch] text-3xl font-semibold leading-[1.15] tracking-tight [animation-delay:60ms] md:text-5xl">
              {g.title}
            </h1>
            <p className="anim-fade-up mt-5 max-w-[65ch] leading-relaxed text-text-dim [animation-delay:120ms]">
              {g.intro}
            </p>
            <nav
              className="anim-fade-up mt-8 flex flex-wrap gap-2 [animation-delay:180ms]"
              aria-label="Table of contents"
            >
              {g.toc.map((t) => (
                <a
                  key={t.id}
                  href={`#${t.id}`}
                  className="rounded-full border border-border px-4 py-1.5 text-sm text-text-dim transition-colors hover:border-accent hover:text-text"
                >
                  {t.label}
                </a>
              ))}
            </nav>
          </div>
        </section>

        {/* 准备工作 */}
        <SectionShell id="prepare">
          <h2 className="reveal text-2xl font-semibold tracking-tight md:text-3xl">
            {g.prepare.title}
          </h2>
          <div className="mt-10 grid grid-cols-1 gap-6 md:grid-cols-3">
            {g.prepare.items.map((item, i) => (
              <div
                key={item.title}
                className="reveal rounded-[12px] border border-border bg-surface-1 p-6"
                style={{ transitionDelay: `${i * 80}ms` }}
              >
                <div className="font-mono text-sm text-accent">{String(i + 1).padStart(2, '0')}</div>
                <h3 className="mt-3 text-lg font-semibold">{item.title}</h3>
                <p className="mt-2 text-sm leading-relaxed text-text-dim">{item.body}</p>
              </div>
            ))}
          </div>
        </SectionShell>

        {/* 手机 → 电脑 */}
        <SectionShell id="phone-to-pc" className="bg-surface-1/40">
          <h2 className="reveal text-2xl font-semibold tracking-tight md:text-3xl">
            {g.phoneToPc.title}
          </h2>
          <p className="reveal mt-4 max-w-[65ch] leading-relaxed text-text-dim">{g.phoneToPc.note}</p>
          <ol className="mt-12 space-y-16">
            {g.phoneToPc.steps.map((s, i) => (
              <li
                key={s.title}
                className="reveal grid grid-cols-1 items-center gap-8 md:grid-cols-2 md:gap-12"
              >
                <div className={i % 2 === 1 ? 'md:order-2' : ''}>
                  <div
                    className={`overflow-hidden rounded-[12px] border border-border bg-surface-1 ${
                      s.kind === 'phone' ? 'mx-auto max-w-[300px]' : ''
                    }`}
                  >
                    <img
                      src={s.imgSrc}
                      alt={s.imgAlt}
                      width={s.kind === 'phone' ? 600 : 1200}
                      height={s.kind === 'phone' ? 1200 : 800}
                      loading="lazy"
                      className="block h-auto w-full"
                    />
                  </div>
                </div>
                <div className={i % 2 === 1 ? 'md:order-1' : ''}>
                  <div className="font-mono text-sm text-accent">{String(i + 1).padStart(2, '0')}</div>
                  <h3 className="mt-3 text-xl font-semibold">{s.title}</h3>
                  <p className="mt-3 max-w-[65ch] text-sm leading-relaxed text-text-dim md:text-base">
                    {s.body}
                  </p>
                </div>
              </li>
            ))}
          </ol>
        </SectionShell>

        {/* 电脑 → 电脑 */}
        <SectionShell id="pc-to-pc">
          <div className="grid grid-cols-1 items-center gap-10 md:grid-cols-[1.1fr_1fr] md:gap-14">
            <div className="reveal">
              <h2 className="text-2xl font-semibold tracking-tight md:text-3xl">{g.pcToPc.title}</h2>
              <p className="mt-5 max-w-[65ch] leading-relaxed text-text-dim">{g.pcToPc.body}</p>
              <ul className="mt-6 space-y-2.5 text-sm">
                {g.pcToPc.points.map((p) => (
                  <li key={p} className="flex items-start gap-2">
                    <Check size={16} className="mt-0.5 shrink-0 text-accent" weight="bold" />
                    <span>{p}</span>
                  </li>
                ))}
              </ul>
            </div>
            <div className="reveal">
              <div className="overflow-hidden rounded-[12px] border border-border bg-surface-1">
                <img
                  src={g.pcToPc.imgSrc}
                  alt={g.pcToPc.imgAlt}
                  width={1200}
                  height={900}
                  loading="lazy"
                  className="block h-auto w-full"
                />
              </div>
            </div>
          </div>
        </SectionShell>

        {/* 设置说明 */}
        <SectionShell id="settings" className="bg-surface-1/40">
          <div className="grid grid-cols-1 items-center gap-10 md:grid-cols-[1fr_1.1fr] md:gap-14">
            <div className="reveal order-2 md:order-1">
              <div className="overflow-hidden rounded-[12px] border border-border bg-surface-1">
                <img
                  src={g.settings.imgSrc}
                  alt={g.settings.imgAlt}
                  width={1200}
                  height={900}
                  loading="lazy"
                  className="block h-auto w-full"
                />
              </div>
            </div>
            <div className="reveal order-1 md:order-2">
              <h2 className="text-2xl font-semibold tracking-tight md:text-3xl">{g.settings.title}</h2>
              <dl className="mt-8 space-y-6">
                {g.settings.points.map(([term, desc]) => (
                  <div key={term}>
                    <dt className="text-sm font-semibold text-accent">{term}</dt>
                    <dd className="mt-1.5 text-sm leading-relaxed text-text-dim">{desc}</dd>
                  </div>
                ))}
              </dl>
            </div>
          </div>
        </SectionShell>

        {/* 常见问题 */}
        <SectionShell id="faq">
          <h2 className="reveal text-2xl font-semibold tracking-tight md:text-3xl">{g.faq.title}</h2>
          <div className="reveal mx-auto mt-10 max-w-3xl space-y-3">
            {g.faq.items.map((item) => (
              <details
                key={item.q}
                className="group rounded-[12px] border border-border bg-surface-1 px-5 py-4"
              >
                <summary className="flex cursor-pointer list-none items-center justify-between gap-4 text-sm font-medium [&::-webkit-details-marker]:hidden">
                  <span>{item.q}</span>
                  <Plus
                    size={16}
                    weight="bold"
                    className="shrink-0 text-text-dim transition-transform group-open:rotate-45"
                  />
                </summary>
                <p className="mt-3 text-sm leading-relaxed text-text-dim">{item.a}</p>
              </details>
            ))}
          </div>
        </SectionShell>

        {/* 收尾 CTA */}
        <SectionShell className="border-t border-border bg-surface-1/40">
          <div className="reveal mx-auto max-w-2xl text-center">
            <h2 className="text-2xl font-semibold tracking-tight md:text-3xl">{g.cta.title}</h2>
            <p className="mt-4 leading-relaxed text-text-dim">{g.cta.body}</p>
            <div className="mt-8 flex flex-wrap items-center justify-center gap-4">
              <Button href={c.links.releases} target="_blank" rel="noreferrer">
                {g.cta.primary}
              </Button>
              <Button variant="ghost" href={g.homeHref}>
                {g.cta.secondary}
              </Button>
            </div>
          </div>
        </SectionShell>
      </main>
      <Footer c={c} />
    </>
  )
}
