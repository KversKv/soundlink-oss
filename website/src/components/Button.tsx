import type { AnchorHTMLAttributes, ReactNode } from 'react'

interface Props extends AnchorHTMLAttributes<HTMLAnchorElement> {
  variant?: 'primary' | 'ghost'
  children: ReactNode
}

export function Button({ variant = 'primary', className = '', children, ...rest }: Props) {
  const base =
    'btn-press inline-flex items-center justify-center gap-2 rounded-[8px] px-5 py-2.5 text-sm font-medium transition-colors'
  const styles =
    variant === 'primary'
      ? 'bg-accent text-[#06231a] hover:bg-[#3ce8b6]'
      : 'border border-border text-text hover:border-text-dim'
  return (
    <a className={`${base} ${styles} ${className}`} {...rest}>
      {children}
    </a>
  )
}
