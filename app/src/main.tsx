import { getCurrentWindow } from '@tauri-apps/api/window'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import MainApp from './MainApp'
import Overlay from './Overlay'

// One bundle serves both windows; the window label picks the UI.
const isOverlay = getCurrentWindow().label === 'overlay'
document.documentElement.classList.add(isOverlay ? 'overlay' : 'main')

createRoot(document.getElementById('root')!).render(
  <StrictMode>{isOverlay ? <Overlay /> : <MainApp />}</StrictMode>,
)
