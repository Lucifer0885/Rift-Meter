import { relaunch } from '@tauri-apps/plugin-process'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { useCallback, useEffect, useRef, useState } from 'react'
import { APP_VERSION } from './brand'

type State =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'none' }
  | { kind: 'available'; update: Update }
  | { kind: 'installing'; done: number; total: number | null }
  | { kind: 'error'; message: string }

/**
 * Checks GitHub Releases for a newer version once at startup (quietly) and on demand.
 * Installing downloads the signed installer, runs it and restarts the app.
 */
function useUpdater() {
  const [state, setState] = useState<State>({ kind: 'idle' })

  const run = useCallback(async (quiet: boolean) => {
    if (!quiet) setState({ kind: 'checking' })
    try {
      const update = await check()
      if (update) setState({ kind: 'available', update })
      else setState(quiet ? { kind: 'idle' } : { kind: 'none' })
    } catch (e) {
      // A failed check at startup (offline, no release yet) isn't worth a message.
      setState(quiet ? { kind: 'idle' } : { kind: 'error', message: String(e) })
    }
  }, [])

  const install = useCallback(async (update: Update) => {
    let done = 0
    let total: number | null = null
    setState({ kind: 'installing', done, total })
    try {
      await update.downloadAndInstall((ev) => {
        if (ev.event === 'Started') total = ev.data.contentLength ?? null
        if (ev.event === 'Progress') done += ev.data.chunkLength
        setState({ kind: 'installing', done, total })
      })
      await relaunch()
    } catch (e) {
      setState({ kind: 'error', message: String(e) })
    }
  }, [])

  const started = useRef(false)
  useEffect(() => {
    if (started.current) return
    started.current = true
    run(true)
  }, [run])

  return { state, check: () => run(false), install, dismiss: () => setState({ kind: 'idle' }) }
}

/** Version badge for the header: click to check for updates; shows the result next to it. */
export function VersionAndUpdates() {
  const { state, check, install, dismiss } = useUpdater()
  const busy = state.kind === 'checking' || state.kind === 'installing'

  return (
    <div className="flex items-center gap-2 text-xs">
      <button
        type="button"
        onClick={check}
        disabled={busy}
        title="Check for updates"
        className="tabular rounded bg-white/5 px-1.5 py-0.5 text-[11px] text-zinc-500 hover:bg-white/10 hover:text-zinc-300 disabled:cursor-default"
      >
        v{APP_VERSION}
      </button>

      {state.kind === 'checking' && <span className="text-zinc-500">Checking for updates…</span>}
      {state.kind === 'none' && (
        <button type="button" onClick={dismiss} className="text-zinc-500 hover:text-zinc-300">
          You have the latest version
        </button>
      )}
      {state.kind === 'available' && (
        <button
          type="button"
          onClick={() => install(state.update)}
          title={state.update.body || undefined}
          className="rounded-md bg-me px-2 py-0.5 font-medium text-white hover:brightness-110"
        >
          Update to v{state.update.version} and restart
        </button>
      )}
      {state.kind === 'installing' && (
        <span className="tabular text-zinc-400">
          Downloading update
          {state.total ? ` ${Math.round((state.done / state.total) * 100)}%` : '…'}
        </span>
      )}
      {state.kind === 'error' && (
        <button type="button" onClick={dismiss} title={state.message} className="text-red-400 hover:text-red-300">
          Update check failed
        </button>
      )}
    </div>
  )
}
