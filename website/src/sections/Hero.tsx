import { Button } from '../components/Button'
import type { Content } from '../content/zh'

export function Hero({ c }: { c: Content }) {
  return (
    <section className="border-b border-border">
      <div className="mx-auto grid w-full max-w-6xl grid-cols-1 items-center gap-12 px-6 py-20 md:min-h-[calc(100dvh-64px)] md:grid-cols-[1.05fr_1fr] md:py-0">
        <div className="anim-fade-up max-w-[65ch]">
          <p className="text-xs font-medium uppercase tracking-[0.18em] text-accent">{c.hero.eyebrow}</p>
          <h1 className="mt-4 text-4xl font-semibold leading-[1.15] tracking-tight md:text-5xl">
            {c.hero.title}
          </h1>
          <p className="mt-5 text-base leading-relaxed text-text-dim md:text-lg">{c.hero.subtitle}</p>
          <div className="mt-8 flex flex-wrap items-center gap-4">
            <Button href={c.links.releases} target="_blank" rel="noreferrer">
              {c.hero.primaryCta}
            </Button>
            <Button variant="ghost" href={c.links.repo} target="_blank" rel="noreferrer">
              {c.hero.secondaryCta}
            </Button>
          </div>
        </div>

        <div className="anim-fade-up [animation-delay:120ms]">
          <div className="overflow-hidden rounded-[12px] border border-border bg-surface-1">
            <img
              src={c.hero.screenshotSrc}
              alt={c.hero.screenshotAlt}
              width={2400}
              height={1500}
              loading="eager"
              fetchPriority="high"
              className="block h-auto w-full"
            />
          </div>
        </div>
      </div>
    </section>
  )
}
