import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useCallback, useEffect, useState } from 'react'
import type { DeviceInfo, HistoryDirInfo, MonitorInfo, OverlayGeometry, SavedFight, Settings, Update } from './types'

export const api = {
  listInterfaces: () => invoke<DeviceInfo[]>('list_interfaces'),
  startCapture: () => invoke<void>('start_capture'),
  stopCapture: () => invoke<void>('stop_capture'),
  getSettings: () => invoke<Settings>('get_settings'),
  setSettings: (settings: Settings) => invoke<void>('set_settings', { settings }),
  resetMeter: () => invoke<void>('reset_meter'),
  clearHistory: () => invoke<void>('clear_history'),
  getUpdate: () => invoke<Update>('get_update'),
  getHistoryDir: () => invoke<HistoryDirInfo>('get_history_dir'),
  listFights: () => invoke<SavedFight[]>('list_fights'),
  deleteFight: (file: string) => invoke<void>('delete_fight', { file }),
  openHistoryDir: () => invoke<void>('open_history_dir'),
  listMonitors: () => invoke<MonitorInfo[]>('list_monitors'),
  getOverlayGeometry: () => invoke<OverlayGeometry>('get_overlay_geometry'),
  setOverlayPosition: (x: number, y: number) =>
    invoke<void>('set_overlay_position', { x: Math.round(x), y: Math.round(y) }),
}

/** Overlay position/size, kept current as it moves or resizes. */
export function useOverlayGeometry(): OverlayGeometry | null {
  const [g, setG] = useState<OverlayGeometry | null>(null)
  useEffect(() => {
    api.getOverlayGeometry().then(setG).catch(() => {})
    const un = listen<OverlayGeometry>('overlay-geometry', (e) => setG(e.payload))
    return () => {
      un.then((f) => f())
    }
  }, [])
  return g
}

/** Saved fights and their folder, reloaded whenever a fight is saved or deleted or the folder changes. */
export function useSavedFights(): { fights: SavedFight[] | null; dir: HistoryDirInfo | null } {
  const [fights, setFights] = useState<SavedFight[] | null>(null)
  const [dir, setDir] = useState<HistoryDirInfo | null>(null)
  useEffect(() => {
    const load = () => {
      api.listFights().then(setFights).catch(() => setFights([]))
      api.getHistoryDir().then(setDir).catch(() => {})
    }
    load()
    const un = listen('fights-changed', load)
    return () => {
      un.then((f) => f())
    }
  }, [])
  return { fights, dir }
}

/** Live numbers, pushed by the backend 5 times a second. */
export function useUpdate(): Update | null {
  const [update, setUpdate] = useState<Update | null>(null)
  useEffect(() => {
    api.getUpdate().then(setUpdate).catch(() => {})
    const un = listen<Update>('update', (e) => setUpdate(e.payload))
    return () => {
      un.then((f) => f())
    }
  }, [])
  return update
}

/** Settings shared by both windows; saving broadcasts them to the other window. */
export function useSettings(): [Settings | null, (s: Settings) => void] {
  const [settings, setLocal] = useState<Settings | null>(null)
  useEffect(() => {
    api.getSettings().then(setLocal).catch(() => {})
    const un = listen<Settings>('settings', (e) => setLocal(e.payload))
    return () => {
      un.then((f) => f())
    }
  }, [])
  const save = useCallback((s: Settings) => {
    setLocal(s)
    api.setSettings(s).catch((e) => console.error(e))
  }, [])
  return [settings, save]
}
