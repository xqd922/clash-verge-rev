import { delayProxyByName } from 'tauri-plugin-mihomo-api'

// 使用节点名作为缓存键，统一延迟显示（符合 mihomo 内核设计）
const hashKey = (name: string) => name

export interface DelayUpdate {
  delay: number
  elapsed?: number
  updatedAt: number
}

const CACHE_TTL = 30 * 60 * 1000

class DelayManager {
  private cache = new Map<string, DelayUpdate>()
  // 全局默认测速 URL + 每组可覆盖
  private globalUrl: string = 'https://www.gstatic.com/generate_204'
  private groupUrls = new Map<string, string>()

  // 每个节点的监听（一个节点可能有多个监听器，来自不同组）
  // key: proxyName, value: Map<listenerId, listener>
  private listenerMap = new Map<
    string,
    Map<string, (update: DelayUpdate) => void>
  >()

  // 每个分组的监听
  private groupListenerMap = new Map<string, () => void>()

  // 设置测速 URL（支持每组独立 URL）
  setUrl(group: string, url: string) {
    if (!url?.trim()) return
    const trimmed = url.trim()
    // 如果与全局默认相同，不需要存储每组 URL
    if (trimmed === this.globalUrl) {
      this.groupUrls.delete(group)
    } else {
      this.groupUrls.set(group, trimmed)
    }
    // 同时更新全局 URL（第一次设置时生效）
    if (
      !this.groupUrls.size ||
      this.globalUrl === 'https://www.gstatic.com/generate_204'
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
    // 使用 group 作为 listenerId，这样同一个节点在不同组都能收到更新
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
    const update: DelayUpdate = {
      delay,
      elapsed: meta?.elapsed,
      updatedAt: Date.now(),
    }

    this.cache.set(key, update)

    // 立即通知所有监听这个节点的监听器
    const listeners = this.listenerMap.get(key)
    if (listeners) {
      listeners.forEach((listener) => {
        try {
          listener(update)
        } catch (error) {
          console.error(`[DelayManager] 通知节点延迟监听器失败: ${name}`, error)
        }
      })
    }

    // 立即通知组监听器
    const groupListener = this.groupListenerMap.get(group)
    if (groupListener) {
      try {
        groupListener()
      } catch (error) {
        console.error(`[DelayManager] 通知分组延迟监听器失败: ${group}`, error)
      }
    }

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
    // 先将状态设置为测试中
    this.setDelay(name, group, -2)

    const startTime = Date.now()

    try {
      const url = this.getUrl(group)

      // 直接调用 mihomo API，超时由 mihomo 内核控制（timeout + 5s）
      const result = await delayProxyByName(name, url, timeout)

      const delay = result.delay
      const elapsed = Date.now() - startTime

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
  }

  async checkListDelay(
    nameList: string[],
    group: string,
    timeout: number,
    concurrency = 36,
    signal?: AbortSignal,
  ) {
    const names = nameList.filter(Boolean)
    // 批量设置正在延迟测试中
    for (const name of names) {
      this.setDelay(name, group, -2)
    }

    let index = 0

    const help = async (): Promise<void> => {
      if (signal?.aborted) return
      const currName = names[index++]
      if (!currName) return

      try {
        await this.checkDelay(currName, group, timeout)
      } catch (ignoreError) {
        // 设置为错误状态
        this.setDelay(currName, group, 1e6)
      }

      return help()
    }

    const actualConcurrency = Math.min(concurrency, names.length)

    const promiseList: Promise<void>[] = []
    for (let i = 0; i < actualConcurrency; i++) {
      promiseList.push(help())
    }

    await Promise.all(promiseList)
  }

  formatDelay(delay: number, timeout = 2000) {
    if (delay === -1) return '-'
    if (delay === -2) return 'testing'
    if (delay === 0 || (delay >= timeout && delay <= 1e5)) return 'Timeout'
    if (delay > 1e5) return 'Error'
    return `${delay}`
  }

  formatDelayColor(delay: number, timeout = 2000) {
    if (delay < 0) return ''
    if (delay === 0 || delay >= timeout) return 'error.main'
    if (delay >= 2000) return 'error.main'
    if (delay >= 400) return 'warning.main'
    if (delay >= 250) return 'primary.main'
    return 'success.main'
  }
}

export default new DelayManager()
