import { useCallback, useRef } from 'react'
import { getProxies, selectNodeForGroup } from 'tauri-plugin-mihomo-api'

import { getProfiles, patchProfile, patchProfilesConfig } from '@/services/cmds'
import {
  revalidateQuery,
  setCacheDataAsync,
  useQuery,
} from '@/services/query-client'
import { debugLog } from '@/utils/debug'

export const useProfiles = () => {
  const {
    data: profiles,
    refetch,
    error,
    isFetching: isValidating,
  } = useQuery({
    queryKey: ['getProfiles'],
    queryFn: async () => {
      const data = await getProfiles()
      debugLog(
        '[useProfiles] 配置数据更新成功，配置数量:',
        data?.items?.length || 0,
      )
      return data
    },
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    staleTime: 500,
    retry: 3,
    retryDelay: 1000,
    refetchInterval: false,
  })

  const refetchRef = useRef(refetch)
  refetchRef.current = refetch
  const mutateProfiles = useCallback(async () => {
    await refetchRef.current()
  }, [])

  const patchProfiles = useCallback(
    async (value: Partial<IProfilesConfig>) => {
      try {
        const outcome = await patchProfilesConfig(value)

        if (outcome.status === 'valid') {
          await setCacheDataAsync<IProfilesConfig>(
            ['getProfiles'],
            (current) => (current ? { ...current, ...value } : current),
          )
        } else if (outcome.status !== 'busy') {
          await mutateProfiles()
        }

        return outcome
      } catch (error) {
        await mutateProfiles()
        throw error
      }
    },
    [mutateProfiles],
  )

  const patchCurrent = useCallback(
    async (value: Partial<IProfileItem>) => {
      if (profiles?.current) {
        await patchProfile(profiles.current, value)
        void mutateProfiles()
      }
    },
    [mutateProfiles, profiles],
  )

  // 根据selected的节点选择
  // targetProfileUid: 显式指定要恢复的目标配置，避免闭包中 profiles.current 是旧值
  const activateSelected = async (
    profileOverride?: IProfilesConfig,
    targetProfileUid?: string,
  ) => {
    try {
      debugLog('[ActivateSelected] 开始处理代理选择')

      const proxiesData = await getProxies()
      const profileData = profileOverride ?? profiles

      if (!profileData || !proxiesData || !profileData.items) {
        debugLog('[ActivateSelected] 代理或配置数据不可用，跳过处理')
        return
      }

      const effectiveCurrent = targetProfileUid ?? profileData.current
      const current = profileData.items?.find(
        (e) => e && e.uid === effectiveCurrent,
      )

      if (!current) {
        debugLog('[ActivateSelected] 未找到当前profile配置')
        return
      }

      // 检查是否有saved的代理选择
      const { selected = [] } = current
      if (selected.length === 0) {
        debugLog('[ActivateSelected] 当前profile无保存的代理选择，跳过')
        return
      }

      type SelectedEntry = { name?: string; now?: string }
      const selectedMap = Object.fromEntries(
        (selected as SelectedEntry[])
          .filter(
            (each): each is SelectedEntry & { name: string; now: string } =>
              each.name != null && each.now != null,
          )
          .map((each) => [each.name, each.now]),
      )

      let hasChange = false
      const newSelected: typeof selected = []
      const proxyRecord: Record<string, any> = proxiesData.proxies ?? {}
      const global = proxyRecord['GLOBAL']
      const groups = Object.values(proxyRecord).filter(
        (g: any) => g?.all && g.name !== 'GLOBAL',
      )
      const selectableTypes = new Set<string>([
        'Selector',
        'URLTest',
        'Fallback',
        'LoadBalance',
      ])

      // 处理所有代理组
      for (const group of [global, ...groups]) {
        if (!group) {
          continue
        }

        const { type, name, now } = group
        const savedProxy = selectedMap[name]
        const availableProxies = Array.isArray(group.all) ? group.all : []

        if (!selectableTypes.has(type as string)) {
          if (savedProxy != null || now != null) {
            const preferredProxy = now ? now : savedProxy
            newSelected.push({ name, now: preferredProxy })
          }
          continue
        }

        if (savedProxy == null) {
          if (now != null) {
            newSelected.push({ name, now })
          }
          continue
        }

        const existsInGroup = (availableProxies as unknown as string[]).some(
          (proxyName) => proxyName === savedProxy,
        )

        if (!existsInGroup) {
          console.warn(
            `[ActivateSelected] 保存的代理 ${savedProxy} 不存在于代理组 ${name}`,
          )
          hasChange = true
          newSelected.push({ name, now: now ?? savedProxy })
          continue
        }

        if (savedProxy !== now) {
          debugLog(
            `[ActivateSelected] 需要切换代理组 ${name}: ${now} -> ${savedProxy}`,
          )
          hasChange = true
          try {
            await selectNodeForGroup(name, savedProxy)
          } catch (error: unknown) {
            console.warn(
              `[ActivateSelected] 切换代理组 ${name} 失败:`,
              error instanceof Error ? error.message : String(error),
            )
          }
        }

        newSelected.push({ name, now: savedProxy })
      }

      if (!hasChange) {
        debugLog('[ActivateSelected] 所有代理选择已经是目标状态，无需更新')
        return
      }

      try {
        await patchProfile(current.uid, { selected: newSelected })
        // 选择已切换，重新拉取代理视图
        await revalidateQuery(['getProxyView'])
      } catch (error: unknown) {
        console.error(
          '[ActivateSelected] 保存代理选择配置失败:',
          error instanceof Error ? error.message : String(error),
        )
      }
    } catch (error: unknown) {
      console.error(
        '[ActivateSelected] 处理代理选择失败:',
        error instanceof Error ? error.message : String(error),
      )
    }
  }

  return {
    profiles,
    current: profiles?.items?.find((p) => p && p.uid === profiles.current),
    activateSelected,
    patchProfiles,
    patchCurrent,
    mutateProfiles,
    // 新增故障检测状态
    isLoading: isValidating,
    error,
    isStale: !profiles && !error && !isValidating, // 检测是否处于异常状态
  }
}
