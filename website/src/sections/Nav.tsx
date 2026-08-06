import { useState } from 'react'
import { List, X } from '@phosphor-icons/react'
import type { Content } from '../content/zh'

export function Nav({ c }: { c: Content }) {
  const [open, setOpen] = useState(false)

  const anchors = [
    { label: c.nav.features, href: '#features' },
    { label: c.nav.how, href: '#how' },
    { label: c.nav.platforms, href: '#platforms' },
    { label: c.nav.specs, href: '#specs' },
  ]

  return (
    <header className="sticky top-0 z-50 border-b border-border bg-bg/85 backdrop-blur">
      <div className="mx-auto flex h-[64px] w-full max-w-6xl items-center justify-between px-6">
        <a href="#" className="flex items-center gap-2 font-semibold tracking-tight" aria-label={c.nav.logoAlt}>
          <img src={`${import.meta.env.BASE_URL}favicon.svg`} alt="" className="h-6 w-6" />
          <span>SoundLink</span>
        </a>

        <nav className="hidden items-center gap-6 text-sm text-text-dim md:flex" aria-label="Primary">
          {anchors.map((a) => (
            <a key={a.href} href={a.href} className="hover:text-text">
              {a.label}
            </a>
          ))}
          <a href={c.links.guide} className="hover:text-text">
            {c.nav.guide}
          </a>
          <a href={c.links.docs} target="_blank" rel="noreferrer" className="hover:text-text">
            {c.nav.docs}
          </a>
        </nav>

        <div className="hidden items-center gap-3 md:flex">
          <a
            href={c.meta.otherLangHref}
            className="rounded-[8px] border border-border px-3 py-1.5 text-sm text-text-dim hover:text-text"
            onClick={() => {
              try {
                localStorage.setItem('soundlink-lang', c.meta.otherLangName === 'English' ? 'en' : 'zh')
              } catch {}
            }}
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

        <button
          className="md:hidden text-text"
          aria-label="Menu"
          aria-expanded={open}
          onClick={() => setOpen((v) => !v)}
        >
          {open ? <X size={22} /> : <List size={22} />}
        </button>
      </div>

      {open && (
        <div className="border-t border-border bg-bg px-6 py-4 md:hidden">
          <nav className="flex flex-col gap-3 text-sm" aria-label="Mobile">
            {anchors.map((a) => (
              <a key={a.href} href={a.href} onClick={() => setOpen(false)}>
                {a.label}
              </a>
            ))}
            <a href={c.links.guide} onClick={() => setOpen(false)}>
              {c.nav.guide}
            </a>
            <a href={c.links.docs} target="_blank" rel="noreferrer">
              {c.nav.docs}
            </a>
            <div className="mt-2 flex items-center gap-3">
              <a href={c.meta.otherLangHref} className="rounded-[8px] border border-border px-3 py-1.5 text-text-dim">
                {c.nav.langSwitch}
              </a>
              <a
                href={c.links.releases}
                target="_blank"
                rel="noreferrer"
                className="btn-press rounded-[8px] bg-accent px-4 py-1.5 font-medium text-[#06231a]"
              >
                {c.nav.cta}
              </a>
            </div>
          </nav>
        </div>
      )}
    </header>
  )
}
