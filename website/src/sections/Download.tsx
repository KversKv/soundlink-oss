import { Button } from '../components/Button'
import { SectionShell } from '../components/SectionShell'
import type { Content } from '../content/zh'

// 全宽居中 CTA 区 + SHA256 / SmartScreen / 实测范围三条说明
export function Download({ c }: { c: Content }) {
  return (
    <SectionShell>
      <div className="reveal mx-auto max-w-2xl text-center">
        <h2 className="text-3xl font-semibold tracking-tight md:text-4xl">{c.download.title}</h2>
        <p className="mt-4 leading-relaxed text-text-dim">{c.download.body}</p>
        <div className="mt-8 flex flex-wrap items-center justify-center gap-4">
          <Button href={c.links.releases} target="_blank" rel="noreferrer">
            {c.download.primaryCta}
          </Button>
          <Button variant="ghost" href={c.links.repo} target="_blank" rel="noreferrer">
            {c.download.secondaryCta}
          </Button>
        </div>
      </div>

      <div className="mx-auto mt-14 grid max-w-4xl grid-cols-1 gap-5 md:grid-cols-3">
        <div className="reveal rounded-[12px] border border-border bg-surface-1 p-6">
          <h3 className="text-base font-semibold">{c.download.shaTitle}</h3>
          <p className="mt-3 text-sm leading-relaxed text-text-dim">{c.download.shaBody}</p>
          <code className="mt-4 block overflow-x-auto rounded-[8px] border border-border bg-bg p-3 font-mono text-xs text-accent">
            {c.download.shaCmd}
          </code>
        </div>
        <div className="reveal rounded-[12px] border border-border bg-surface-1 p-6" style={{ transitionDelay: '80ms' }}>
          <h3 className="text-base font-semibold">{c.download.smartScreenTitle}</h3>
          <p className="mt-3 text-sm leading-relaxed text-text-dim">{c.download.smartScreenBody}</p>
        </div>
        <div className="reveal rounded-[12px] border border-border bg-surface-1 p-6" style={{ transitionDelay: '160ms' }}>
          <h3 className="text-base font-semibold">{c.download.testedTitle}</h3>
          <p className="mt-3 text-sm leading-relaxed text-text-dim">{c.download.testedBody}</p>
        </div>
      </div>
    </SectionShell>
  )
}
