import { delayProxyByName } from 'tauri-plugin-mihomo-api'

import { debugLog } from '@/utils/debug'

// Use proxy name as cache key, unified delay display (matches mihomo core design)
const hashKey = (name: string) => name

export interface DelayUpdate {
  delay: number
  elapsed?: number
  updatedAt: number
}

const CACHE_TTL = 30 * 60 * 1000

class DelayManager {
  private cache = new Map<string, DelayUpdate>()
  // Global default test URL + per-group overrides
  private globalUrl: string = 'http://www.gstatic.com/generate_204'
  private groupUrls = new Map<string, string>()

  // Per-node listeners (a node can have multiple listeners from different groups)
  // key: proxyName, value: Map<listenerId, listener>
  private listenerMap = new Map<
    string,
    Map<string, (update: DelayUpdate) => void>
  >()

  // 每个分组的监听
  private groupListenerMap = new Map<string, () => void>()

  private pendingItemUpdates = new Map<string, DelayUpdate[]>()
  private pendingGroupUpdates = new Set<string>()
  private itemFlushScheduled = false
  private groupFlushScheduled = false

  private scheduleItemFlush() {
    if (this.itemFlushScheduled) return
    this.itemFlushScheduled = true

    const run = () => {
      this.itemFlushScheduled = false
      const updates = this.pendingItemUpdates
      this.pendingItemUpdates = new Map()

      updates.forEach((queue, proxyName) => {
        const listeners = this.listenerMap.get(proxyName)
        if (!listeners) return

        queue.forEach((update) => {
          // Notify all listeners watching this node
          listeners.forEach((listener, listenerId) => {
            try {
              listener(update)
            } catch (error) {
              console.error(
                `[DelayManager] 通知节点延迟监听器失败: ${proxyName}:${listenerId}`,
                error,
              )
            }
          })
        })
      })
    }

    if (typeof window !== 'undefined') {
      if (typeof window.requestAnimationFrame === 'function') {
        window.requestAnimationFrame(run)
        return
      }
      if (typeof window.setTimeout === 'function') {
        window.setTimeout(run, 0)
        return
      }
    }

    Promise.resolve().then(run)
  }

  private scheduleGroupFlush() {
    if (this.groupFlushScheduled) return
    this.groupFlushScheduled = true

    const run = () => {
      this.groupFlushScheduled = false
      const groups = this.pendingGroupUpdates
      this.pendingGroupUpdates = new Set()

      groups.forEach((group) => {
        const listener = this.groupListenerMap.get(group)
        if (!listener) return
        try {
          listener()
        } catch (error) {
          console.error(
            `[DelayManager] 通知分组延迟监听器失败: ${group}`,
            error,
          )
        }
      })
    }

    if (typeof window !== 'undefined') {
      if (typeof window.requestAnimationFrame === 'function') {
        window.requestAnimationFrame(run)
        return
      }
      if (typeof window.setTimeout === 'function') {
        window.setTimeout(run, 0)
        return
      }
    }

    Promise.resolve().then(run)
  }

  private queueGroupNotification(group: string) {
    this.pendingGroupUpdates.add(group)
    this.scheduleGroupFlush()
  }

  // 设置测速 URL（支持每组独立 URL）
  setUrl(group: string, url: string) {
    if (!url?.trim()) return
    const trimmed = url.trim()
    // If same as global default, no need to store per-group URL
    if (trimmed === this.globalUrl) {
      this.groupUrls.delete(group)
    } else {
      this.groupUrls.set(group, trimmed)
    }
    // Also update global URL (takes effect on first set)
    if (
      !this.groupUrls.size ||
      this.globalUrl === 'http://www.gstatic.com/generate_204'
    ) {
      this.globalUrl = trimmed
    }
  }

  getUrl(group?: string) {
    const url = (group && this.groupUrls.get(group)) || this.globalUrl
    return url
  }

  setListener(
    name: string,
    group: string,
    listener: (update: DelayUpdate) => void,
  ) {
    // Use group as listenerId so the same node in different groups receives updates
    const listenerId = group
    let listeners = this.listenerMap.get(name)
    if (!listeners) {
      listeners = new Map()
      this.listenerMap.set(name, listeners)
    }
    listeners.set(listenerId, listener)
  }

  removeListener(name: string, group: string) {
    const listenerId = group
    const listeners = this.listenerMap.get(name)
    if (listeners) {
      listeners.delete(listenerId)
      if (listeners.size === 0) {
        this.listenerMap.delete(name)
      }
    }
  }

  setGroupListener(group: string, listener: () => void) {
    this.groupListenerMap.set(group, listener)
  }

  removeGroupListener(group: string) {
    this.groupListenerMap.delete(group)
  }

  setDelay(
    name: string,
    group: string,
    delay: number,
    meta?: { elapsed?: number },
  ): DelayUpdate {
    const key = hashKey(name)
    debugLog(
      `[DelayManager] 设置延迟，代理: ${name}, 组: ${group}, 延迟: ${delay}`,
    )
    const update: DelayUpdate = {
      delay,
      elapsed: meta?.elapsed,
      updatedAt: Date.now(),
    }

    this.cache.set(key, update)

    const queue = this.pendingItemUpdates.get(key)
    if (queue) {
      queue.push(update)
    } else {
      this.pendingItemUpdates.set(key, [update])
    }
    this.scheduleItemFlush()

    return update
  }

  getDelayUpdate(name: string, _group: string) {
    const key = hashKey(name)
    const entry = this.cache.get(key)
    if (!entry) return undefined

    if (Date.now() - entry.updatedAt > CACHE_TTL) {
      this.cache.delete(key)
      return undefined
    }

    return { ...entry }
  }

  getDelay(name: string, group: string) {
    const update = this.getDelayUpdate(name, group)
    return update ? update.delay : -1
  }

  /// 暂时修复provider的节点延迟排序的问题
  getDelayFix(proxy: IProxyItem, group: string) {
    if (!proxy.provider) {
      const update = this.getDelayUpdate(proxy.name, group)
      if (update && (update.delay >= 0 || update.delay === -2)) {
        return update.delay
      }
    }

    // 添加 history 属性的安全检查
    if (proxy.history && proxy.history.length > 0) {
      // 0ms以error显示
      return proxy.history[proxy.history.length - 1].delay || 1e6
    }
    return -1
  }

  async checkDelay(
    name: string,
    group: string,
    timeout: number,
  ): Promise<DelayUpdate> {
    debugLog(
      `[DelayManager] 开始测试延迟，代理: ${name}, 组: ${group}, 超时: ${timeout}ms`,
    )

    // 先将状态设置为测试中
    this.setDelay(name, group, -2)

    const startTime = Date.now()

    try {
      const url = this.getUrl(group)
      debugLog(`[DelayManager] 调用API测试延迟，代理: ${name}, URL: ${url}`)

      // Call mihomo API directly, timeout controlled by mihomo core (timeout + 5s)
      const result = await delayProxyByName(name, url, timeout)

      const delay = result.delay
      const elapsed = Date.now() - startTime
      debugLog(`[DelayManager] 延迟测试完成，代理: ${name}, 结果: ${delay}ms`)

      return this.setDelay(name, group, delay, { elapsed })
    } catch (error) {
      console.error(`[DelayManager] 延迟测试出错，代理: ${name}`, error)
      const delay = 1e6 // error
      const elapsed = Date.now() - startTime

      return this.setDelay(name, group, delay, { elapsed })
    }
  }

  // 从 delayGroup API 的返回结果批量更新延迟缓存
  // delayGroup 返回的结果中不包含超时的节点，需要传入完整列表来标记超时
  batchSetDelay(
    result: Record<string, number>,
    allNames: string[],
    group: string,
  ) {
    for (const name of allNames) {
      const delay = result[name]
      if (delay !== undefined && delay > 0) {
        this.setDelay(name, group, delay)
      } else {
        // 不在结果中的节点 = 超时（mihomo delay_group 不返回超时节点）
        this.setDelay(name, group, 0)
      }
    }
    this.queueGroupNotification(group)
    debugLog(
      `[DelayManager] 批量设置延迟完成，组: ${group}, 成功: ${Object.keys(result).length}, 总数: ${allNames.length}`,
    )
  }

  async checkListDelay(
    nameList: string[],
    group: string,
    timeout: number,
    concurrency = 36,
    signal?: AbortSignal,
  ) {
    debugLog(
      `[DelayManager] 批量测试延迟开始，组: ${group}, 数量: ${nameList.length}, 并发数: ${concurrency}`,
    )
    const names = nameList.filter(Boolean)

    let index = 0
    const startTime = Date.now()
    const listener = this.groupListenerMap.get(group)

    const help = async (): Promise<void> => {
      if (signal?.aborted) return
      const currName = names[index++]
      if (!currName) return

      try {
        await this.checkDelay(currName, group, timeout)
        if (listener) {
          this.queueGroupNotification(group)
        }
      } catch (error) {
        console.error(
          `[DelayManager] 批量测试单个代理出错，代理: ${currName}`,
          error,
        )
        // 设置为错误状态
        this.setDelay(currName, group, 1e6)
      }

      return help()
    }

    const actualConcurrency = Math.min(concurrency, names.length)
    debugLog(`[DelayManager] 实际并发数: ${actualConcurrency}`)

    const promiseList: Promise<void>[] = []
    for (let i = 0; i < actualConcurrency; i++) {
      promiseList.push(help())
    }

    await Promise.all(promiseList)
    const totalTime = Date.now() - startTime
    debugLog(
      `[DelayManager] 批量测试延迟完成，组: ${group}, 总耗时: ${totalTime}ms`,
    )
  }

  formatDelay(delay: number, timeout = 10000) {
    if (delay === -1) return '-'
    if (delay === -2) return 'testing'
    if (delay === 0 || (delay >= timeout && delay <= 1e5)) return 'Timeout'
    if (delay > 1e5) return 'Error'
    return `${delay}`
  }

  formatDelayColor(delay: number, timeout = 10000) {
    if (delay < 0) return ''
    if (delay === 0 || delay >= timeout) return 'error.main'
    if (delay >= 10000) return 'error.main'
    if (delay >= 400) return 'warning.main'
    if (delay >= 250) return 'primary.main'
    return 'success.main'
  }
}

export default new DelayManager()
