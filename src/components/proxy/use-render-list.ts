import { useEffect, useMemo, useRef } from 'react'

import { useVerge } from '@/hooks/use-verge'
import { useAppRefreshers, useProxiesData } from '@/providers/app-data-context'

import { filterSort } from './use-filter-sort'
import {
  DEFAULT_STATE,
  useHeadStateNew,
  type HeadState,
} from './use-head-state'
import { useWindowWidth } from './use-window-width'

// 定义代理项接口
interface IProxyItem {
  name: string
  type: string
  udp: boolean
  xudp: boolean
  tfo: boolean
  mptcp: boolean
  smux: boolean
  history: {
    time: string
    delay: number
  }[]
  provider?: string
  testUrl?: string
  [key: string]: any // 添加索引签名以适应其他可能的属性
}

// 代理组类型
type ProxyGroup = {
  name: string
  type: string
  udp: boolean
  xudp: boolean
  tfo: boolean
  mptcp: boolean
  smux: boolean
  history: {
    time: string
    delay: number
  }[]
  now: string
  all: IProxyItem[]
  hidden?: boolean
  icon?: string
  testUrl?: string
  provider?: string
  fixed?: string
}

export interface IRenderItem {
  // 组 | head | item | empty | item col
  type: 0 | 1 | 2 | 3 | 4
  key: string
  group: ProxyGroup
  proxy?: IProxyItem
  col?: number
  proxyCol?: IProxyItem[]
  headState?: HeadState
  // 新增支持图标和其他元数据
  icon?: string
  provider?: string
  testUrl?: string
}

type GroupCache = {
  now: string
  all: IProxyItem[]
  headState: HeadState
  col: number
  latencyTimeout: number | undefined
  items: IRenderItem[]
}

// 优化列布局计算
const calculateColumns = (width: number, configCol: number): number => {
  if (configCol > 0 && configCol < 6) return configCol

  if (width > 1920) return 5
  if (width > 1450) return 4
  if (width > 1024) return 3
  if (width > 900) return 2
  if (width >= 600) return 2
  return 1
}

// 优化分组逻辑
const groupProxies = <T = any>(list: T[], size: number): T[][] => {
  return list.reduce((acc, item) => {
    const lastGroup = acc[acc.length - 1]
    if (!lastGroup || lastGroup.length >= size) {
      acc.push([item])
    } else {
      lastGroup.push(item)
    }
    return acc
  }, [] as T[][])
}

export const useRenderList = (mode: string) => {
  // 使用全局数据提供者
  const { proxies: proxiesData } = useProxiesData()
  const { refreshProxy } = useAppRefreshers()
  const { verge } = useVerge()
  const { width } = useWindowWidth()
  const [headStates, setHeadState] = useHeadStateNew()
  const latencyTimeout = verge?.default_latency_timeout

  // 计算列数
  const col = useMemo(
    () => calculateColumns(width, verge?.proxy_layout_column || 6),
    [width, verge?.proxy_layout_column],
  )

  // 确保代理数据加载
  useEffect(() => {
    if (!proxiesData) return
    const { groups, proxies } = proxiesData

    if (
      (mode === 'rule' && !groups.length) ||
      (mode === 'global' && proxies.length < 2)
    ) {
      const handle = setTimeout(() => refreshProxy(), 500)
      return () => clearTimeout(handle)
    }
  }, [proxiesData, mode, refreshProxy])

  const groupCacheRef = useRef<Map<string, GroupCache>>(new Map())
  const prevListRef = useRef<IRenderItem[]>([])

  // 处理渲染列表
  const renderList: IRenderItem[] = useMemo(() => {
    if (!proxiesData) return []

    // 正常模式的渲染逻辑
    const useRule = mode === 'rule' || mode === 'script'
    const renderGroups =
      useRule && proxiesData.groups.length
        ? proxiesData.groups
        : [proxiesData.global!]

    const cache = groupCacheRef.current
    let anyChanged = false

    const retList = renderGroups.flatMap((group: ProxyGroup) => {
      const headState = headStates[group.name] || DEFAULT_STATE
      const cached = cache.get(group.name)

      if (
        cached &&
        cached.now === group.now &&
        cached.all === group.all &&
        cached.headState === headState &&
        cached.col === col &&
        cached.latencyTimeout === latencyTimeout
      ) {
        return cached.items
      }

      anyChanged = true
      const ret: IRenderItem[] = [
        {
          type: 0,
          key: group.name,
          group,
          headState,
          icon: group.icon,
          testUrl: group.testUrl,
        },
      ]

      if (headState?.open || !useRule) {
        const proxies = filterSort(
          group.all,
          group.name,
          headState.filterText,
          headState.sortType,
          latencyTimeout,
          {
            matchCase: headState.filterMatchCase,
            matchWholeWord: headState.filterMatchWholeWord,
            useRegularExpression: headState.filterUseRegularExpression,
          },
        )

        ret.push({
          type: 1,
          key: `head-${group.name}`,
          group,
          headState,
        })

        if (!proxies.length) {
          ret.push({
            type: 3,
            key: `empty-${group.name}`,
            group,
            headState,
          })
        } else if (col > 1) {
          ret.push(
            ...groupProxies(proxies, col).map((proxyCol, colIndex) => ({
              type: 4 as const,
              key: `col-${group.name}-${colIndex}`,
              group,
              headState,
              col,
              proxyCol,
              provider: proxyCol[0].provider,
            })),
          )
        } else {
          ret.push(
            ...proxies.map((proxy) => ({
              type: 2 as const,
              key: `${group.name}-${proxy!.name}`,
              group,
              proxy,
              headState,
              provider: proxy.provider,
            })),
          )
        }
      }

      cache.set(group.name, {
        now: group.now,
        all: group.all,
        headState,
        col,
        latencyTimeout,
        items: ret,
      })
      return ret
    })

    const filtered = !useRule
      ? retList.slice(1)
      : retList.filter((item: IRenderItem) => !item.group.hidden)

    if (!anyChanged && prevListRef.current.length === filtered.length) {
      return prevListRef.current
    }
    prevListRef.current = filtered
    return filtered
  }, [headStates, proxiesData, mode, col, latencyTimeout])

  return {
    renderList,
    onProxies: refreshProxy,
    onHeadState: setHeadState,
    currentColumns: col,
  }
}

// 优化建议：如有大数据量，建议用虚拟滚动（已在 ProxyGroups 组件中实现），此处无需额外处理。
