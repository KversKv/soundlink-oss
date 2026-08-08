import { SectionShell } from '../components/SectionShell'
import type { Content } from '../content/zh'

// Pro 亮点区：快速分辨率切换（QR-1）。左文案右双截图（设置面板 + 托盘菜单交叠）。
export function QuickResolution({ c }: { c: Content }) {
  const q = c.quickResolution
  return (
    <SectionShell>
      <div className="grid grid-cols-1 items-center gap-12 md:grid-cols-[1fr_1.1fr]">
        <div className="reveal max-w-[65ch]">
          <span className="inline-flex items-center rounded-[8px] border border-border bg-surface-2 px-2 py-0.5 text-xs font-medium text-text-dim">
            {q.badge}
          </span>
          <h2 className="mt-4 text-2xl font-semibold tracking-tight md:text-3xl">{q.title}</h2>
          <p className="mt-5 leading-relaxed text-text-dim">{q.body}</p>
          <ul className="mt-6 space-y-2.5 text-sm text-text-dim">
            {q.points.map((p) => (
              <li key={p} className="flex items-start gap-2.5">
                <span className="mt-[9px] h-1 w-1 shrink-0 rounded-full bg-accent" aria-hidden="true" />
                <span>{p}</span>
              </li>
            ))}
          </ul>
          <p className="mt-6 text-xs leading-relaxed text-text-dim/70">{q.note}</p>
        </div>

        <div className="reveal relative mb-12 md:mb-0">
          <div className="overflow-hidden rounded-[12px] border border-border bg-surface-1 shadow-2xl shadow-black/40">
            <img
              src={q.settingsShotSrc}
              alt={q.settingsShotAlt}
              width={612}
              height={500}
              loading="lazy"
              className="block h-auto w-full"
            />
          </div>
          <div className="absolute -bottom-10 right-2 w-[52%] overflow-hidden rounded-[12px] border border-border bg-surface-1 shadow-2xl shadow-black/60 md:-right-6">
            <img
              src={q.trayShotSrc}
              alt={q.trayShotAlt}
              width={722}
              height={162}
              loading="lazy"
              className="block h-auto w-full"
            />
          </div>
        </div>
      </div>
    </SectionShell>
  )
}
