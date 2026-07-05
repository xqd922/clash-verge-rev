import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useMemo } from 'react'
import { getSmartWeights } from 'tauri-plugin-mihomo-api'

interface RawNodeRankItem {
  Name: string
  Rank: string
  Weight: number
}

export interface SmartTopNode {
  name: string
  weight: number
}

// 上游 Rank 分类原始值
export type SmartRank = 'MostUsed' | 'OccasionalUsed' | 'RarelyUsed'

// 内核枚举 -> i18n key (展示用词跟随 Surge: Most Used / Often Used / Sometimes)
export const SMART_RANK_I18N_KEY = {
  MostUsed: 'proxies.page.labels.smartRank.MostUsed',
  OccasionalUsed: 'proxies.page.labels.smartRank.OccasionalUsed',
  RarelyUsed: 'proxies.page.labels.smartRank.RarelyUsed',
} as const

export interface UseSmartWeights {
  // 折叠组头展示: 权重最高的 Top 1 节点
  topNodes: SmartTopNode[]
  // 每个节点名 -> Rank 分类
  rankMap: Map<string, SmartRank>
}

const EMPTY: UseSmartWeights = { topNodes: [], rankMap: new Map() }
// 组头只展示权重最高的节点，和普通核心的 now 一样保持一行
const MAX_HEADER_NODES = 1

// Smart core exposes a per-group node weight ranking via GET /group/{name}/weights.
// The ranking reflects the dynamic scoring the smart algorithm uses to pick a node
// per connection — there is no single stable "now" node for a smart group.
export function parseSmartWeights(data: unknown): UseSmartWeights {
  const rawList = (data as { weights?: RawNodeRankItem[] } | undefined)?.weights

  if (!Array.isArray(rawList) || rawList.length === 0) {
    return EMPTY
  }

  const rankMap = new Map<string, SmartRank>()
  for (const item of rawList) {
    if (item?.Name && item.Rank) {
      rankMap.set(item.Name, item.Rank as SmartRank)
    }
  }

  const topNodes = rawList
    .filter((item) => item?.Name)
    .sort((a, b) => b.Weight - a.Weight)
    .slice(0, MAX_HEADER_NODES)
    .map((item) => ({ name: item.Name, weight: item.Weight }))

  return { topNodes, rankMap }
}

export function useSmartWeights(
  groupName: string,
  enabled: boolean,
): UseSmartWeights {
  const queryClient = useQueryClient()
  const profileId = queryClient.getQueryData<IProfilesConfig>([
    'getProfiles',
  ])?.current

  const { data } = useQuery({
    queryKey: ['smartWeights', groupName, profileId],
    queryFn: () => getSmartWeights(groupName),
    enabled,
    refetchInterval: 5000,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  })

  return useMemo(() => parseSmartWeights(data), [data])
}
