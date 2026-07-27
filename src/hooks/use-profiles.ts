import { useQuery } from '@tanstack/react-query'
import { useCallback, useRef } from 'react'

import { getProfiles, patchProfile, patchProfilesConfig } from '@/services/cmds'
import { setCacheDataAsync } from '@/services/query-client'
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

  return {
    profiles,
    current: profiles?.items?.find((profile) =>
      profile ? profile.uid === profiles.current : false,
    ),
    patchProfiles,
    patchCurrent,
    mutateProfiles,
    isLoading: isValidating,
    error,
    isStale: !profiles && !error && !isValidating,
  }
}
