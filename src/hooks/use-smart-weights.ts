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

// Rank 优先级：MostUsed > OccasionalUsed > RarelyUsed
// Weight 在不同 Rank 下含义不同（MostUsed=综合评分，RarelyUsed=连接成功率%）
// 必须先按 Rank 优先级排序，再按 Weight 排序
const RANK_PRIORITY: Record<string, number> = {
  MostUsed: 3,
  OccasionalUsed: 2,
  RarelyUsed: 1,
}

export function formatSmartTopNode(node?: SmartTopNode): string {
  return node ? `${node.name} ${Math.round(node.weight)}` : ''
}

export function getProxyNowLabel(
  proxyType: string,
  proxyNow?: string,
  smartTopNode?: SmartTopNode,
): string | undefined {
  return proxyType === 'Smart'
    ? formatSmartTopNode(smartTopNode) || undefined
    : proxyNow
}

// Smart core exposes a per-group node weight ranking via GET /group/{name}/weights.
// The ranking reflects the dynamic scoring the smart algorithm uses to pick a node
// per connection — there is no single stable "now" node for a smart group.
export function parseSmartWeights(
  data: unknown,
  proxyNames?: string[],
): UseSmartWeights {
  const rawList = (data as { weights?: RawNodeRankItem[] } | undefined)?.weights

  if (!Array.isArray(rawList) || rawList.length === 0) {
    return EMPTY
  }

  // proxyNames 为空时不做过滤 —— 但如果没有传入，也不应显示全部权重
  // 只有明确传入非空列表时才过滤；否则返回空（避免显示旧配置的节点）
  if (!proxyNames?.length) {
    return EMPTY
  }

  const allowedNames = new Set(proxyNames)
  const currentList = rawList.filter(
    (item) => item?.Name && allowedNames.has(item.Name),
  )

  if (currentList.length === 0) {
    return EMPTY
  }

  const rankMap = new Map<string, SmartRank>()
  for (const item of currentList) {
    if (item?.Name && item.Rank) {
      rankMap.set(item.Name, item.Rank as SmartRank)
    }
  }

  const topNodes = currentList
    .filter((item) => item?.Name)
    .sort((a, b) => {
      // 先按 Rank 优先级排序（MostUsed > OccasionalUsed > RarelyUsed）
      const rankDiff =
        (RANK_PRIORITY[b.Rank] ?? 0) - (RANK_PRIORITY[a.Rank] ?? 0)
      if (rankDiff !== 0) return rankDiff
      // 同 Rank 内按 Weight 排序
      return b.Weight - a.Weight
    })
    .slice(0, MAX_HEADER_NODES)
    .map((item) => ({ name: item.Name, weight: item.Weight }))

  return { topNodes, rankMap }
}

export function useSmartWeights(
  groupName: string,
  enabled: boolean,
  proxyNames?: string[],
): UseSmartWeights {
  const queryClient = useQueryClient()
  const profileId = queryClient.getQueryData<IProfilesConfig>([
    'getProfiles',
  ])?.current

  // 用代理组节点列表特征作为 queryKey 的一部分
  // 当配置切换后代理组节点变化时，自动触发 refetch
  const proxyKey = proxyNames?.length
    ? `${proxyNames.length}:${proxyNames[0] ?? ''}:${proxyNames[proxyNames.length - 1] ?? ''}`
    : 'empty'

  const { data } = useQuery({
    queryKey: ['smartWeights', groupName, profileId, proxyKey],
    queryFn: () => getSmartWeights(groupName),
    enabled,
    refetchInterval: 5000,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  })

  return useMemo(() => parseSmartWeights(data, proxyNames), [data, proxyNames])
}
