import type { Content } from '../content/zh'

type Status = 'tested' | 'ready' | 'planned'

const DOT: Record<Status, string> = {
  tested: 'bg-accent',
  ready: 'bg-[#e5c07b]',
  planned: 'bg-[#e06c75]',
}

export function StatusBadge({
  status,
  labels,
}: {
  status: Status
  labels: Content['platforms']['status']
}) {
  return (
    <span className="inline-flex items-center gap-1.5 rounded-[8px] border border-border bg-surface-2 px-2 py-0.5 text-xs text-text-dim">
      <span className={`inline-block h-1.5 w-1.5 rounded-full ${DOT[status]}`} aria-hidden="true" />
      {labels[status]}
    </span>
  )
}
