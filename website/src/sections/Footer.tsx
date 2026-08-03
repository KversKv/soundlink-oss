import type { Content } from '../content/zh'

export function Footer({ c }: { c: Content }) {
  return (
    <footer className="border-t border-border bg-surface-1/40">
      <div className="mx-auto w-full max-w-6xl px-6 py-14">
        <div className="grid grid-cols-1 gap-10 md:grid-cols-[1.2fr_repeat(3,1fr)]">
          <div>
            <div className="flex items-center gap-2 font-semibold">
              <img src={`${import.meta.env.BASE_URL}favicon.svg`} alt="" className="h-6 w-6" />
              <span>SoundLink</span>
            </div>
            <p className="mt-3 max-w-[65ch] text-sm text-text-dim">{c.footer.tagline}</p>
          </div>
          {c.footer.columns.map((col) => (
            <div key={col.name}>
              <h3 className="text-sm font-semibold">{col.name}</h3>
              <ul className="mt-4 space-y-2.5 text-sm text-text-dim">
                {col.links.map((l) => (
                  <li key={l.label}>
                    <a href={l.href} target="_blank" rel="noreferrer" className="hover:text-text">
                      {l.label}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
        <div className="mt-12 border-t border-border pt-6 text-xs text-text-dim">{c.footer.copyright}</div>
      </div>
    </footer>
  )
}
