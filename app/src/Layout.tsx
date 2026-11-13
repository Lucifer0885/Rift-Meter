import { useEffect, useMemo, useRef, useState } from 'react'
import { api, useOverlayGeometry } from './api'
import type { MonitorInfo, OverlayGeometry, Settings } from './types'
import { APP_NAME } from './brand'
import { Button, Card, Toggle } from './ui'

const GRID_SIZES = [0, 20, 50, 100] // physical pixels; 0 = no grid
const EDGE_MARGIN = 20 // px from the screen edge for the quick positions

function containsCenter(m: MonitorInfo, g: OverlayGeometry) {
  const cx = g.x + g.width / 2
  const cy = g.y + g.height / 2
  return cx >= m.x && cx < m.x + m.width && cy >= m.y && cy < m.y + m.height
}

function clamp(v: number, lo: number, hi: number) {
  return Math.min(Math.max(v, lo), Math.max(lo, hi))
}

/**
 * A scaled copy of the chosen monitor (same aspect ratio) with the overlay drawn on it.
 * Dragging the box moves the real overlay window.
 */
export default function Layout({ settings, save }: { settings: Settings; save: (s: Settings) => void }) {
  const geometry = useOverlayGeometry()
  const [monitors, setMonitors] = useState<MonitorInfo[]>([])
  const [monIndex, setMonIndex] = useState<number | null>(null)
  const [grid, setGrid] = useState(50)
  const [snap, setSnap] = useState(true)
  const [canvasWidth, setCanvasWidth] = useState(800)
  const [drag, setDrag] = useState<{ x: number; y: number } | null>(null) // live position while dragging
  const canvasRef = useRef<HTMLDivElement>(null)
  const grab = useRef<{ dx: number; dy: number } | null>(null)
  const pending = useRef<{ x: number; y: number } | null>(null)
  const frame = useRef<number | null>(null)

  useEffect(() => {
    api.listMonitors().then(setMonitors).catch(() => {})
  }, [])

  // Default to the display the overlay is on, else the primary one.
  useEffect(() => {
    if (monIndex !== null || monitors.length === 0 || !geometry) return
    const onIt = monitors.findIndex((m) => containsCenter(m, geometry))
    const primary = monitors.findIndex((m) => m.primary)
    setMonIndex(onIt >= 0 ? onIt : Math.max(primary, 0))
  }, [monitors, geometry, monIndex])

  useEffect(() => {
    const el = canvasRef.current
    if (!el) return
    const ro = new ResizeObserver(() => setCanvasWidth(el.clientWidth))
    ro.observe(el)
    return () => ro.disconnect()
  }, [monIndex])

  const mon = monIndex !== null ? monitors[monIndex] : undefined
  const scale = mon ? canvasWidth / mon.width : 1

  // Overlay position relative to the chosen monitor, in physical px.
  const pos = useMemo(() => {
    if (drag) return drag
    if (!mon || !geometry) return null
    return { x: geometry.x - mon.x, y: geometry.y - mon.y }
  }, [drag, mon, geometry])

  const onThisMonitor = !!mon && !!geometry && containsCenter(mon, geometry)

  const move = (x: number, y: number, useSnap = snap) => {
    if (!mon || !geometry) return
    let nx = clamp(x, 0, mon.width - geometry.width)
    let ny = clamp(y, 0, mon.height - geometry.height)
    if (useSnap && grid > 0) {
      nx = clamp(Math.round(nx / grid) * grid, 0, mon.width - geometry.width)
      ny = clamp(Math.round(ny / grid) * grid, 0, mon.height - geometry.height)
    }
    // Send at most one move per animation frame.
    pending.current = { x: mon.x + nx, y: mon.y + ny }
    if (frame.current === null) {
      frame.current = requestAnimationFrame(() => {
        frame.current = null
        if (pending.current) api.setOverlayPosition(pending.current.x, pending.current.y)
      })
    }
    return { x: nx, y: ny }
  }

  const onPointerDown = (e: React.PointerEvent) => {
    if (!pos || !canvasRef.current) return
    const r = canvasRef.current.getBoundingClientRect()
    grab.current = { dx: (e.clientX - r.left) / scale - pos.x, dy: (e.clientY - r.top) / scale - pos.y }
    ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
    setDrag(pos)
  }

  const onPointerMove = (e: React.PointerEvent) => {
    if (!grab.current || !canvasRef.current) return
    const r = canvasRef.current.getBoundingClientRect()
    const p = move((e.clientX - r.left) / scale - grab.current.dx, (e.clientY - r.top) / scale - grab.current.dy)
    if (p) setDrag(p)
  }

  const onPointerUp = () => {
    grab.current = null
    // Keep the dragged spot until the window's own move event catches up.
    setTimeout(() => setDrag(null), 150)
  }

  const quick = (h: 'l' | 'c' | 'r', v: 't' | 'm' | 'b') => {
    if (!mon || !geometry) return
    const x = h === 'l' ? EDGE_MARGIN : h === 'r' ? mon.width - geometry.width - EDGE_MARGIN : (mon.width - geometry.width) / 2
    const y = v === 't' ? EDGE_MARGIN : v === 'b' ? mon.height - geometry.height - EDGE_MARGIN : (mon.height - geometry.height) / 2
    move(x, y, false) // exact positions for presets
  }

  const setOverlay = (patch: Partial<Settings['overlay']>) => save({ ...settings, overlay: { ...settings.overlay, ...patch } })

  const gridStep = grid * scale
  const gridStyle =
    grid > 0 && gridStep >= 4
      ? {
          backgroundImage:
            'linear-gradient(to right, rgba(255,255,255,0.06) 1px, transparent 1px),' +
            'linear-gradient(to bottom, rgba(255,255,255,0.06) 1px, transparent 1px)',
          backgroundSize: `${gridStep}px ${gridStep}px`,
        }
      : {}

  return (
    <div className="space-y-4">
      <Card
        title="Overlay position"
        right={
          <div className="flex items-center gap-3 text-sm">
            {monitors.length > 1 && (
              <select
                value={monIndex ?? 0}
                onChange={(e) => setMonIndex(Number(e.target.value))}
                className="rounded-md border border-white/10 bg-zinc-900 px-2 py-1"
              >
                {monitors.map((m, i) => (
                  <option key={i} value={i}>
                    {m.primary ? 'Main display' : `Display ${i + 1}`} — {m.width}×{m.height}
                  </option>
                ))}
              </select>
            )}
            <label className="flex items-center gap-2 text-zinc-400">
              Grid
              <select
                value={grid}
                onChange={(e) => setGrid(Number(e.target.value))}
                className="rounded-md border border-white/10 bg-zinc-900 px-2 py-1 text-zinc-200"
              >
                {GRID_SIZES.map((g) => (
                  <option key={g} value={g}>
                    {g === 0 ? 'Off' : `${g}px`}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex items-center gap-2 text-zinc-400">
              <input type="checkbox" checked={snap} onChange={(e) => setSnap(e.target.checked)} />
              Snap
            </label>
          </div>
        }
      >
        {!mon ? (
          <p className="text-sm text-zinc-400">Reading your displays…</p>
        ) : (
          <>
            <div
              ref={canvasRef}
              className="relative w-full overflow-hidden rounded-md border border-white/10 bg-zinc-950"
              style={{ aspectRatio: `${mon.width} / ${mon.height}`, ...gridStyle }}
            >
              <span className="pointer-events-none absolute right-2 bottom-1.5 text-[11px] text-zinc-600">
                {mon.width}×{mon.height}
                {mon.scale !== 1 && ` · ${Math.round(mon.scale * 100)}% scaling`}
              </span>
              {geometry && pos && onThisMonitor ? (
                <div
                  onPointerDown={onPointerDown}
                  onPointerMove={onPointerMove}
                  onPointerUp={onPointerUp}
                  className={`absolute flex cursor-grab touch-none flex-col justify-center rounded border px-2 select-none active:cursor-grabbing ${
                    settings.overlay.visible ? 'border-me bg-me/25' : 'border-dashed border-zinc-500 bg-zinc-700/30'
                  }`}
                  style={{
                    left: pos.x * scale,
                    top: pos.y * scale,
                    width: geometry.width * scale,
                    height: geometry.height * scale,
                  }}
                >
                  {geometry.height * scale > 28 && (
                    <>
                      <span className="truncate text-[11px] font-semibold text-white">{APP_NAME} overlay</span>
                      <span className="tabular truncate text-[10px] text-zinc-300">
                        x {Math.round(pos.x)}, y {Math.round(pos.y)}
                      </span>
                    </>
                  )}
                </div>
              ) : (
                <div className="absolute inset-0 flex items-center justify-center">
                  <Button
                    variant="primary"
                    onClick={() => geometry && mon && api.setOverlayPosition(mon.x + EDGE_MARGIN, mon.y + EDGE_MARGIN)}
                  >
                    Move the overlay to this display
                  </Button>
                </div>
              )}
            </div>
            <p className="mt-2 text-xs text-zinc-500">
              This box is your screen at {Math.round(scale * 1000) / 10}% size. Drag the blue box and the real
              overlay follows.
            </p>
          </>
        )}
      </Card>

      <div className="grid grid-cols-2 gap-4">
        <Card title="Quick positions">
          <div className="grid w-40 grid-cols-3 gap-1.5">
            {(['t', 'm', 'b'] as const).flatMap((v) =>
              (['l', 'c', 'r'] as const).map((h) => (
                <button
                  key={v + h}
                  onClick={() => quick(h, v)}
                  className="h-9 rounded bg-white/5 text-xs text-zinc-400 hover:bg-white/10 hover:text-zinc-100"
                  title={`${{ t: 'Top', m: 'Middle', b: 'Bottom' }[v]} ${{ l: 'left', c: 'center', r: 'right' }[h]}`}
                >
                  {{ t: '↑', m: '·', b: '↓' }[v]}
                  {{ l: '←', c: '', r: '→' }[h]}
                </button>
              )),
            )}
          </div>
        </Card>
        <Card title="Overlay">
          <Toggle label="Show overlay" checked={settings.overlay.visible} onChange={(v) => setOverlay({ visible: v })} />
          <Toggle
            label="Lock in place"
            hint="Stops dragging or resetting by clicking the overlay. Same as its lock icon. You can still move it here."
            checked={settings.overlay.locked}
            onChange={(v) => setOverlay({ locked: v })}
          />
          {pos && mon && (
            <div className="tabular mt-2 flex gap-3 text-sm">
              {(['x', 'y'] as const).map((axis) => (
                <label key={axis} className="flex items-center gap-2 text-zinc-400">
                  {axis.toUpperCase()}
                  <input
                    type="number"
                    value={Math.round(pos[axis])}
                    onChange={(e) => {
                      const v = Number(e.target.value)
                      move(axis === 'x' ? v : pos.x, axis === 'y' ? v : pos.y, false)
                    }}
                    className="w-24 rounded-md border border-white/10 bg-zinc-900 px-2 py-1 text-zinc-200"
                  />
                </label>
              ))}
            </div>
          )}
        </Card>
      </div>
    </div>
  )
}
