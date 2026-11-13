import { useEffect, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { APP_NAME, Logo } from './brand'
import { classColor } from './format'
import { classIcon } from './skillIcons'
import type { Update } from './types'
import { VersionAndUpdates } from './updater'

/**
 * The main window's own frame (decorations are off in tauri.conf.json): app name on the left,
 * your character in the middle, window buttons on the right. Every element marked
 * data-tauri-drag-region drags the window (and double-click maximizes); the attribute only
 * counts on the element itself, so the plain-text children carry it too.
 */
export function TitleBar({ update }: { update: Update | null }) {
  const win = getCurrentWindow()
  const [maximized, setMaximized] = useState(false)
  useEffect(() => {
    let stop: (() => void) | undefined
    const check = () => win.isMaximized().then(setMaximized).catch(() => {})
    check()
    win.onResized(check).then((un) => (stop = un))
    return () => stop?.()
  }, [win])

  return (
    <div data-tauri-drag-region className="flex h-9 shrink-0 items-center border-b border-white/5 bg-black/20 select-none">
      <div data-tauri-drag-region className="flex min-w-0 flex-1 items-center gap-2 pl-3">
        <Logo className="pointer-events-none h-4 w-4" />
        <span data-tauri-drag-region className="text-xs font-semibold tracking-tight text-zinc-200">
          {APP_NAME}
        </span>
        <VersionAndUpdates />
      </div>
      <div data-tauri-drag-region className="flex min-w-0 shrink items-center justify-center px-3">
        <Player update={update} />
      </div>
      <div data-tauri-drag-region className="flex h-full flex-1 items-stretch justify-end">
        <WinButton title="Minimize" onClick={() => win.minimize()}>
          <path d="M1 5.5h9" />
        </WinButton>
        <WinButton title={maximized ? 'Restore' : 'Maximize'} onClick={() => win.toggleMaximize()}>
          {maximized ? (
            <>
              <rect x="1" y="3" width="6.5" height="6.5" />
              <path d="M3 3V1h6.5v6.5H7.5" />
            </>
          ) : (
            <rect x="1" y="1" width="9" height="9" />
          )}
        </WinButton>
        <WinButton title="Close" danger onClick={() => win.close()}>
          <path d="M1 1l9 9M10 1l-9 9" />
        </WinButton>
      </div>
    </div>
  )
}

/** Your character (class icon, name, level), or a hint until a map load tells us. */
function Player({ update }: { update: Update | null }) {
  const p = update?.snapshot.player
  if (!p) {
    return update?.status.running ? (
      <span data-tauri-drag-region className="truncate text-xs text-zinc-500" title="Your character is read when a map loads">
        Change map to find your character
      </span>
    ) : null
  }
  const icon = p.class ? classIcon(p.class) : undefined
  const title = [p.class, p.level ? `level ${p.level}` : null].filter(Boolean).join(' · ')
  return (
    <span data-tauri-drag-region className="flex min-w-0 items-center gap-1.5 text-xs" title={title || undefined}>
      {icon && <img src={icon} alt={p.class ?? ''} className="pointer-events-none h-4 w-4 shrink-0 rounded-sm" draggable={false} />}
      <span
        data-tauri-drag-region
        className="truncate font-semibold"
        style={{ color: (p.class && classColor(p.class)) || undefined }}
      >
        {p.name ?? p.class}
      </span>
      {p.level ? (
        <span data-tauri-drag-region className="text-zinc-500">
          {p.level}
        </span>
      ) : null}
    </span>
  )
}

function WinButton({
  title,
  danger,
  onClick,
  children,
}: {
  title: string
  danger?: boolean
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      title={title}
      onClick={onClick}
      className={`flex w-11 items-center justify-center text-zinc-400 transition-colors ${
        danger ? 'hover:bg-red-600 hover:text-white' : 'hover:bg-white/10 hover:text-zinc-100'
      }`}
    >
      <svg viewBox="0 0 11 11" className="h-2.5 w-2.5" fill="none" stroke="currentColor" strokeWidth="1">
        {children}
      </svg>
    </button>
  )
}
