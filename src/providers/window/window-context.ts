import { getCurrentWindow } from '@tauri-apps/api/window'
import { createContext } from 'react'

export interface WindowContextType {
  maximized: boolean | null
  minimize: () => Promise<void>
  close: () => Promise<void>
  toggleMaximize: () => Promise<void>
  toggleFullscreen: () => Promise<void>
  currentWindow: ReturnType<typeof getCurrentWindow>
}

export const WindowContext = createContext<WindowContextType | undefined>(
  undefined,
)
