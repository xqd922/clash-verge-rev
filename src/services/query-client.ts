import { QueryClient } from '@tanstack/react-query'

type CacheKey = string | readonly unknown[]
type CacheDataUpdater<T> =
  | T
  | undefined
  | ((current: T | undefined) => T | undefined)

const normalizeCacheKey = (queryKey: CacheKey): readonly unknown[] =>
  typeof queryKey === 'string' ? [queryKey] : queryKey

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 2000,
      retry: 3,
      retryDelay: 5000,
      refetchOnWindowFocus: false,
    },
  },
})

export const getCacheData = <T>(queryKey: CacheKey): T | undefined =>
  queryClient.getQueryData<T>(normalizeCacheKey(queryKey))

export const setCacheData = <T>(
  queryKey: CacheKey,
  updaterOrData: CacheDataUpdater<T>,
): T | undefined => {
  const normalizedKey = normalizeCacheKey(queryKey)
  const current = queryClient.getQueryData<T>(normalizedKey)
  const next =
    typeof updaterOrData === 'function'
      ? (updaterOrData as (value: T | undefined) => T | undefined)(current)
      : updaterOrData

  if (next === undefined) {
    queryClient.removeQueries({ queryKey: normalizedKey, exact: true })
  } else {
    queryClient.setQueryData<T>(normalizedKey, next)
  }

  return next
}

export const setCacheDataAsync = async <T>(
  queryKey: CacheKey,
  updaterOrData: CacheDataUpdater<T>,
): Promise<T | undefined> => setCacheData(queryKey, updaterOrData)

export const revalidateQuery = async <T = unknown>(
  queryKey: CacheKey,
): Promise<T | undefined> => {
  const normalizedKey = normalizeCacheKey(queryKey)
  await queryClient.invalidateQueries({
    queryKey: normalizedKey,
    exact: true,
  })
  return queryClient.getQueryData<T>(normalizedKey)
}

export const revalidateQueries = (queryKeys: readonly CacheKey[]) =>
  Promise.all(queryKeys.map((queryKey) => revalidateQuery(queryKey)))

export const removeCacheData = async (queryKey: CacheKey): Promise<void> => {
  queryClient.removeQueries({
    queryKey: normalizeCacheKey(queryKey),
    exact: true,
  })
}
