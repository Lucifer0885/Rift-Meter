import { useEffect, useState } from 'react'
import { api } from './api'
import type { Settings } from './types'
import { Button } from './ui'

const MODIFIER_CODES = /^(Control|Shift|Alt|Meta|OS)(Left|Right)?$/

/** KeyboardEvent.code -> the key name the backend understands ("KeyO" -> "O", "Digit1" -> "1"). */
const keyName = (code: string) => code.replace(/^Key(?=[A-Z]$)/, '').replace(/^Digit(?=\d$)/, '')

/** "Ctrl+Shift+O" from a key press, or null while only modifiers are held. */
function comboFrom(e: KeyboardEvent): string | null {
  if (MODIFIER_CODES.test(e.code) || !e.code) return null
  const mods = [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift', e.metaKey && 'Super'].filter(Boolean)
  return [...mods, keyName(e.code)].join('+')
}

/** A plain key without Ctrl/Alt/Super would fire while typing in game chat, so only F-keys may stand alone. */
const needsModifier = (combo: string) => !/(Ctrl|Alt|Super)\+/.test(combo) && !/(^|\+)F\d{1,2}$/.test(combo)

/** Shows the overlay hotkey and records a new one: click, then press the keys. */
export function HotkeyField({ settings }: { settings: Settings }) {
  const [recording, setRecording] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const current = settings.overlayHotkey

  const apply = (overlayHotkey: string | null) => {
    setError(null)
    // Saved straight away (not through useSettings) so a rejected key never shows as set.
    api.setSettings({ ...settings, overlayHotkey }).catch((e) => setError(String(e)))
  }

  useEffect(() => {
    if (!recording) return
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      if (e.code === 'Escape' && !e.ctrlKey && !e.altKey && !e.shiftKey) {
        setRecording(false)
        return
      }
      const combo = comboFrom(e)
      if (!combo) return
      if (needsModifier(combo)) {
        setError('Add Ctrl, Alt or Win (or use an F-key), so the key still works for typing in game.')
        return
      }
      setRecording(false)
      apply(combo)
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [recording, settings])

  return (
    <div className="py-1.5">
      <div className="text-sm text-zinc-200">Show/hide shortcut</div>
      <div className="text-xs text-zinc-500">Works while the game is focused.</div>
      <div className="mt-1.5 flex items-center gap-2">
        <button
          type="button"
          onClick={() => {
            setError(null)
            setRecording(!recording)
          }}
          className={`min-w-40 rounded-md border px-3 py-1.5 text-left font-mono text-sm transition-colors ${
            recording ? 'border-me bg-me/15 text-zinc-100' : 'border-white/10 bg-black/30 text-zinc-200 hover:bg-white/5'
          }`}
        >
          {recording ? 'Press keys… (Esc cancels)' : current || 'Not set'}
        </button>
        {current && !recording && <Button onClick={() => apply(null)}>Clear</Button>}
      </div>
      {error && <div className="mt-1 text-xs text-red-400">{error}</div>}
    </div>
  )
}
