import { useQuery } from '@tanstack/react-query'

import { queryClient } from '@/services/query-client'
import { checkUpdateSafe } from '@/services/update'

import { useVerge } from './use-verge'

export interface UpdateInfo {
  version: string
  body: string
  date: string
  available: boolean
  downloadAndInstall: (onEvent?: any) => Promise<void>
}

const LAST_CHECK_KEY = 'last_check_update'

export const readLastCheckTime = (): number | null => {
  const stored = localStorage.getItem(LAST_CHECK_KEY)
  if (!stored) return null
  const ts = parseInt(stored, 10)
  return isNaN(ts) ? null : ts
}

export const updateLastCheckTime = (timestamp?: number): number => {
  const now = timestamp ?? Date.now()
  localStorage.setItem(LAST_CHECK_KEY, now.toString())
  queryClient.setQueryData([LAST_CHECK_KEY], now)
  return now
}

// --- useUpdate hook ---

export type UpdateCheckMode = 'auto' | 'manual'

export const useUpdate = (mode: UpdateCheckMode = 'auto') => {
  const { verge } = useVerge()
  const { auto_check_update } = verge || {}

  // auto：跟随「自动检查更新」开关；manual：从不主动请求，
  // 仅展示手动「检查更新」按钮写入缓存的结果
  const shouldCheck = mode === 'auto' && auto_check_update !== false

  const {
    data: updateInfo,
    refetch: checkUpdate,
    isFetching: isValidating,
  } = useQuery({
    queryKey: ['checkUpdate'],
    queryFn: async () => {
      const result = await checkUpdateSafe()
      updateLastCheckTime()
      return result
    },
    enabled: shouldCheck,
    retry: 2,
    staleTime: 60 * 60 * 1000,
    refetchInterval: mode === 'auto' ? 24 * 60 * 60 * 1000 : false,
    refetchOnWindowFocus: false,
  })

  // Shared last check timestamp
  const { data: lastCheckUpdate } = useQuery({
    queryKey: [LAST_CHECK_KEY],
    queryFn: readLastCheckTime,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  })

  return {
    updateInfo,
    checkUpdate,
    loading: isValidating,
    lastCheckUpdate: lastCheckUpdate ?? null,
  }
}
