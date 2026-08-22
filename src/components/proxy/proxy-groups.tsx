import { useTheme } from '@mui/material/styles'
import { useQuery } from '@tanstack/react-query'
import { defaultRangeExtractor, useVirtualizer } from '@tanstack/react-virtual'
import {
  type Key,
  type RefObject,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { useLocation } from 'react-router'
import { healthcheckProxyProvider, unfixedProxy } from 'tauri-plugin-mihomo-api'

import { BaseEmpty } from '@/components/base'
import { useProxySelection } from '@/hooks/use-proxy-selection'
import { useVerge } from '@/hooks/use-verge'
import { calcuProxies } from '@/services/cmds'
import delayManager from '@/services/delay'

import { ScrollTopButton } from '../layout/scroll-top-button'

import {
  DEFAULT_HOVER_DELAY,
  ProxyGroupNavigator,
} from './proxy-group-navigator'
import { ProxyRender } from './proxy-render'
import type { HeadState } from './use-head-state'
import { type IRenderItem, useRenderList } from './use-render-list'

function useStableCallback<T extends (...args: any[]) => any>(fn: T): T {
  const ref = useRef(fn)
  ref.current = fn
  return useCallback((...args: Parameters<T>) => ref.current(...args), []) as T
}

interface Props {
  mode: string
}

export const ProxyGroups = (props: Props) => {
  const { pathname } = useLocation()
  const { mode } = props

  // Cold start: poll every 1s until data arrives, then back off to 3s
  useQuery({
    queryKey: ['getProxies'],
    queryFn: calcuProxies,
    refetchInterval: (query) => (query.state.data ? 3000 : 1000),
    refetchIntervalInBackground: false,
    staleTime: 1500,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  })

  const { verge } = useVerge()

  const { renderList, onProxies, onHeadState } = useRenderList(mode)

  // 统代理选择
  const { handleProxyGroupChange } = useProxySelection({
    onSuccess: () => {
      onProxies()
    },
    onError: (error) => {
      console.error('代理切换失败', error)
      onProxies()
    },
  })

  const timeout = verge?.default_latency_timeout || 10000

  const parentRef = useRef<HTMLDivElement>(null)
  const scrollPositionRef = useRef<Record<string, number>>({})
  const scrollTopRef = useRef(0)
  const showScrollTopRef = useRef(false)
  const activeStickyIndexRef = useRef<number | null>(null)
  const restoredScrollKeyRef = useRef<string | null>(null)
  const [showScrollTop, setShowScrollTop] = useState(false)
  const scrollPositionKey = useMemo(() => `${mode}:normal`, [mode])
  const stickyGroupIndexes = useMemo(
    () =>
      renderList.flatMap((item, index) =>
        item.type === 0 && !item.group.hidden ? [index] : [],
      ),
    [renderList],
  )

  const rangeExtractor = useCallback(
    (range: Parameters<typeof defaultRangeExtractor>[0]) => {
      const activeStickyIndex = [...stickyGroupIndexes]
        .reverse()
        .find((index) => index <= range.startIndex)
      activeStickyIndexRef.current = activeStickyIndex ?? null

      const indexes = defaultRangeExtractor(range)
      return activeStickyIndex == null || indexes.includes(activeStickyIndex)
        ? indexes
        : [activeStickyIndex, ...indexes]
    },
    [stickyGroupIndexes],
  )

  const virtualizer = useVirtualizer({
    count: renderList.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 56,
    overscan: 15,
    getItemKey: (index) => renderList[index]?.key ?? index,
    rangeExtractor,
  })
  const virtualItems = virtualizer.getVirtualItems()
  const activeStickyIndex = activeStickyIndexRef.current

  // 从 localStorage 恢复滚动位置
  useLayoutEffect(() => {
    if (renderList.length === 0) return
    const node = parentRef.current
    if (!node) return
    if (
      restoredScrollKeyRef.current === scrollPositionKey &&
      node.scrollTop === scrollTopRef.current
    ) {
      return
    }

    try {
      const savedPositions = localStorage.getItem('proxy-scroll-positions')
      if (savedPositions) {
        const positions = JSON.parse(savedPositions)
        scrollPositionRef.current = positions
        const savedPosition = positions[scrollPositionKey]

        if (savedPosition !== undefined) {
          node.scrollTop = savedPosition
          scrollTopRef.current = savedPosition
          const nextShowScrollTop = savedPosition > 100
          showScrollTopRef.current = nextShowScrollTop
          queueMicrotask(() => setShowScrollTop(nextShowScrollTop))
        }
      }
    } catch (e) {
      console.error('Error restoring scroll position:', e)
    }
    restoredScrollKeyRef.current = scrollPositionKey
  }, [pathname, renderList.length, scrollPositionKey])

  // 改为使用节流函数保存滚动位置
  const saveScrollPosition = useCallback(
    (scrollTop: number) => {
      try {
        scrollPositionRef.current[scrollPositionKey] = scrollTop
        localStorage.setItem(
          'proxy-scroll-positions',
          JSON.stringify(scrollPositionRef.current),
        )
      } catch (e) {
        console.error('Error saving scroll position:', e)
      }
    },
    [scrollPositionKey],
  )

  const saveScrollPositionThrottled = useMemo(
    () => throttle(saveScrollPosition, 500),
    [saveScrollPosition],
  )

  const handleScroll = useCallback(
    (event: Event) => {
      const target = event.target as HTMLElement | null
      const nextScrollTop = target?.scrollTop ?? 0
      const nextShowScrollTop = nextScrollTop > 100
      scrollTopRef.current = nextScrollTop

      if (showScrollTopRef.current !== nextShowScrollTop) {
        showScrollTopRef.current = nextShowScrollTop
        setShowScrollTop(nextShowScrollTop)
      }

      saveScrollPositionThrottled(nextScrollTop)
    },
    [saveScrollPositionThrottled],
  )

  // 添加和清理滚动事件监听器
  useEffect(() => {
    const node = parentRef.current
    if (!node) return

    const listener = handleScroll as EventListener
    const options: AddEventListenerOptions = { passive: true }

    node.addEventListener('scroll', listener, options)

    return () => {
      if (restoredScrollKeyRef.current === scrollPositionKey) {
        saveScrollPosition(scrollTopRef.current)
      }
      node.removeEventListener('scroll', listener, options)
    }
  }, [handleScroll, saveScrollPosition, scrollPositionKey])

  // 滚动到顶部
  const scrollToTop = useCallback(() => {
    parentRef.current?.scrollTo?.({
      top: 0,
      behavior: 'smooth',
    })
    scrollTopRef.current = 0
    saveScrollPosition(0)
  }, [saveScrollPosition])

  const handleChangeProxy = useCallback(
    (group: IProxyGroupItem, proxy: IProxyItem) => {
      if (!['Selector', 'URLTest', 'Fallback', 'Smart'].includes(group.type)) {
        return
      }

      handleProxyGroupChange(group, proxy)
    },
    [handleProxyGroupChange],
  )

  const getGroupHeadState = useCallback(
    (groupName: string) => {
      const headItem = renderList.find(
        (item) => item.type === 1 && item.group?.name === groupName,
      )
      return headItem?.headState
    },
    [renderList],
  )

  const checkAllAbortRef = useRef<AbortController | null>(null)
  const handleCheckAll = useStableCallback(async (groupName: string) => {
    checkAllAbortRef.current?.abort()
    const abortController = new AbortController()
    checkAllAbortRef.current = abortController
    let completed = false

    const proxies = renderList
      .filter(
        (item) =>
          item.group?.name === groupName &&
          (item.type === 2 || item.type === 4),
      )
      .flatMap((item) => item.proxyCol || item.proxy!)
      .filter(Boolean)

    const providers = new Set(
      proxies.map((proxy) => proxy!.provider!).filter(Boolean),
    )

    if (providers.size) {
      Promise.allSettled(
        [...providers].map((provider) => healthcheckProxyProvider(provider)),
      ).then(() => onProxies())
    }

    const names = proxies
      .filter((proxy) => !proxy!.provider)
      .map((proxy) => proxy!.name)

    const group = renderList.find(
      (item) => item.type === 0 && item.group?.name === groupName,
    )?.group
    if (group?.now) {
      const index = names.indexOf(group.now)
      if (index > 0) {
        names.unshift(names.splice(index, 1)[0])
      }
    }

    if (group?.fixed) {
      await unfixedProxy(groupName).catch(() => {})
    }

    try {
      await delayManager.checkListDelay(
        names,
        groupName,
        timeout,
        36,
        abortController.signal,
      )
      completed = true
    } catch (error) {
      console.warn(
        `[ProxyGroups] Delay test failed for group: ${groupName}`,
        error,
      )
    } finally {
      const isCurrentRun = checkAllAbortRef.current === abortController
      if (isCurrentRun && (completed || !abortController.signal.aborted)) {
        if (group?.type === 'URLTest') {
          await unfixedProxy(groupName).catch(() => {})
        }
        const headState = getGroupHeadState(groupName)
        if (headState?.sortType === 1) {
          onHeadState(groupName, { sortType: headState.sortType })
        }
        onProxies()
      }
      if (isCurrentRun) {
        checkAllAbortRef.current = null
      }
    }
  })

  // 滚到对应的节点
  const handleLocation = useStableCallback((group: IProxyGroupItem) => {
    if (!group) return
    const { name, now } = group

    const index = renderList.findIndex(
      (e) =>
        e.group?.name === name &&
        ((e.type === 2 && e.proxy?.name === now) ||
          (e.type === 4 && e.proxyCol?.some((p) => p.name === now))),
    )

    if (index >= 0) {
      virtualizer.scrollToIndex(index, { align: 'center', behavior: 'smooth' })
    }
  })

  // 定位到指定的代理组
  const handleGroupLocationByName = useCallback(
    (groupName: string) => {
      const index = renderList.findIndex(
        (item) => item.type === 0 && item.group?.name === groupName,
      )

      if (index >= 0) {
        virtualizer.scrollToIndex(index, { align: 'start', behavior: 'smooth' })
      }
    },
    [renderList, virtualizer],
  )

  const proxyGroupNames = useMemo(() => {
    const names = renderList
      .filter((item) => item.type === 0 && item.group?.name)
      .map((item) => item.group!.name)
    return Array.from(new Set(names))
  }, [renderList])

  const renderProxyList = (height: string) => (
    <ProxyVirtualList
      parentRef={parentRef}
      height={height}
      totalSize={virtualizer.getTotalSize()}
      virtualItems={virtualItems}
      renderList={renderList}
      activeStickyIndex={activeStickyIndex}
      indent={mode === 'rule' || mode === 'script'}
      measureElement={virtualizer.measureElement}
      onLocation={handleLocation}
      onCheckAll={handleCheckAll}
      onHeadState={onHeadState}
      onChangeProxy={handleChangeProxy}
    />
  )

  if (mode === 'direct') {
    return <BaseEmpty textKey="proxies.page.messages.directMode" />
  }

  return (
    <div
      style={{ position: 'relative', height: '100%', willChange: 'transform' }}
    >
      {/* 代理组导航栏 */}
      {mode === 'rule' && (
        <ProxyGroupNavigator
          proxyGroupNames={proxyGroupNames}
          onGroupLocation={handleGroupLocationByName}
          enableHoverJump={verge?.enable_hover_jump_navigator ?? true}
          hoverDelay={verge?.hover_jump_navigator_delay ?? DEFAULT_HOVER_DELAY}
        />
      )}

      {renderProxyList('calc(100% - 14px)')}
      <ScrollTopButton show={showScrollTop} onClick={scrollToTop} />
    </div>
  )
}

type VirtualListItem = {
  key: Key
  index: number
  start: number
  end: number
}

interface ProxyVirtualListProps {
  parentRef: RefObject<HTMLDivElement | null>
  height: string
  totalSize: number
  virtualItems: VirtualListItem[]
  renderList: IRenderItem[]
  activeStickyIndex: number | null
  indent: boolean
  measureElement: (node: Element | null) => void
  onLocation: (group: IRenderItem['group']) => void
  onCheckAll: (groupName: string) => void
  onHeadState: (groupName: string, patch: Partial<HeadState>) => void
  onChangeProxy: (
    group: IRenderItem['group'],
    proxy: IRenderItem['proxy'] & { name: string },
  ) => void
}

function ProxyVirtualList({
  parentRef,
  height,
  totalSize,
  virtualItems,
  renderList,
  activeStickyIndex,
  indent,
  measureElement,
  onLocation,
  onCheckAll,
  onHeadState,
  onChangeProxy,
}: ProxyVirtualListProps) {
  const theme = useTheme()
  const stickyBackground =
    theme.palette.mode === 'dark' ? '#1e1f27' : 'var(--background-color)'

  return (
    <div ref={parentRef} style={{ height, overflow: 'auto' }}>
      <div style={{ height: totalSize, position: 'relative' }}>
        {virtualItems.map((virtualItem) => (
          <div
            key={virtualItem.key}
            data-index={virtualItem.index}
            ref={measureElement}
            style={{
              position:
                virtualItem.index === activeStickyIndex ? 'sticky' : 'absolute',
              top: 0,
              left: 0,
              zIndex: virtualItem.index === activeStickyIndex ? 5 : undefined,
              display:
                virtualItem.index === activeStickyIndex
                  ? 'flow-root'
                  : undefined,
              backgroundColor:
                virtualItem.index === activeStickyIndex
                  ? stickyBackground
                  : undefined,
              width: '100%',
              transform:
                virtualItem.index === activeStickyIndex
                  ? undefined
                  : `translateY(${virtualItem.start}px)`,
            }}
          >
            <ProxyRender
              item={renderList[virtualItem.index]}
              indent={indent}
              onLocation={onLocation}
              onCheckAll={onCheckAll}
              onHeadState={onHeadState}
              onChangeProxy={onChangeProxy}
            />
          </div>
        ))}
        <div style={{ height: 8 }} />
      </div>
    </div>
  )
}

// 替换简单防抖函数为更优的节流函数
function throttle<T extends (...args: any[]) => any>(
  func: T,
  wait: number,
): (...args: Parameters<T>) => void {
  let timer: ReturnType<typeof setTimeout> | null = null
  let previous = 0
  let lastArgs: Parameters<T> | null = null

  const run = (args: Parameters<T>) => {
    previous = Date.now()
    timer = null
    lastArgs = null
    func(...args)
  }

  return function (...args: Parameters<T>) {
    const now = Date.now()
    const remaining = wait - (now - previous)
    lastArgs = args

    if (remaining <= 0 || remaining > wait) {
      if (timer) {
        clearTimeout(timer)
      }
      run(args)
    } else if (!timer) {
      timer = setTimeout(() => {
        if (lastArgs) {
          run(lastArgs)
        }
      }, remaining)
    }
  }
}
