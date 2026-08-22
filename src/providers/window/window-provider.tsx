import { getCurrentWindow } from '@tauri-apps/api/window'
import React, { useCallback, useEffect, useMemo, useState } from 'react'

import debounce from '@/utils/debounce'
import getSystem from '@/utils/get-system'

import { WindowContext } from './window-context'

export const WindowProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const currentWindow = useMemo(() => getCurrentWindow(), [])
  // 按平台默认值乐观初始化，避免首帧无标题栏、IPC 返回后内容整体下移
  const [decorated, setDecorated] = useState<boolean | null>(
    () => getSystem() === 'macos',
  )
  const [maximized, setMaximized] = useState<boolean | null>(null)

  const close = useCallback(async () => {
    // Delay one frame so the UI can clear :hover before the window hides.
    await new Promise((resolve) => setTimeout(resolve, 20))
    await currentWindow.close()
  }, [currentWindow])
  const minimize = useCallback(async () => {
    // Delay one frame so the UI can clear :hover before the window hides.
    await new Promise((resolve) => setTimeout(resolve, 10))
    await currentWindow.minimize()
  }, [currentWindow])

  useEffect(() => {
    let isUnmounted = false
    let lastWidth = -1
    let lastHeight = -1

    const checkMaximized = debounce(
      async (event: { payload: { width: number; height: number } }) => {
        if (isUnmounted) return
        const { width, height } = event.payload
        if (width === lastWidth && height === lastHeight) return
        lastWidth = width
        lastHeight = height
        const value = await currentWindow.isMaximized()
        setMaximized(value)
      },
      300,
    )

    const unlistenPromise = currentWindow.onResized(checkMaximized)

    return () => {
      isUnmounted = true
      unlistenPromise
        .then((unlisten) => unlisten())
        .catch((err) => console.warn('[WindowProvider] 清理监听器失败:', err))
    }
  }, [currentWindow])

  const toggleMaximize = useCallback(async () => {
    if (await currentWindow.isMaximized()) {
      await currentWindow.unmaximize()
      setMaximized(false)
    } else {
      await currentWindow.maximize()
      setMaximized(true)
    }
  }, [currentWindow])

  const toggleFullscreen = useCallback(async () => {
    await currentWindow.setFullscreen(!(await currentWindow.isFullscreen()))
  }, [currentWindow])

  const refreshDecorated = useCallback(async () => {
    const val = await currentWindow.isDecorated()
    setDecorated(val)
    return val
  }, [currentWindow])

  const toggleDecorations = useCallback(async () => {
    const currentVal = await currentWindow.isDecorated()
    await currentWindow.setDecorations(!currentVal)
    setDecorated(!currentVal)
  }, [currentWindow])

  useEffect(() => {
    refreshDecorated()
    currentWindow.setMinimizable?.(true)
  }, [currentWindow, refreshDecorated])

  const contextValue = useMemo(
    () => ({
      decorated,
      maximized,
      toggleDecorations,
      refreshDecorated,
      minimize,
      close,
      toggleMaximize,
      toggleFullscreen,
      currentWindow,
    }),
    [
      decorated,
      maximized,
      toggleDecorations,
      refreshDecorated,
      minimize,
      close,
      toggleMaximize,
      toggleFullscreen,
      currentWindow,
    ],
  )

  return <WindowContext value={contextValue}>{children}</WindowContext>
}
