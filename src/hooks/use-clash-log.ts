import { useLocalStorage } from 'foxact/use-local-storage'
import type { LogLevel } from 'tauri-plugin-mihomo-api'

export type StoredLogLevel = Lowercase<LogLevel> | 'warn'

interface ClashLogState {
  enable: boolean
  logLevel: StoredLogLevel
  logFilter: LogFilter
  logOrder: LogOrder
}

const defaultClashLog: ClashLogState = {
  enable: true,
  logLevel: 'info',
  logFilter: 'all',
  logOrder: 'asc',
}

export const useClashLog = () =>
  useLocalStorage<ClashLogState>('clash-log', defaultClashLog, {
    serializer: JSON.stringify,
    deserializer: JSON.parse,
  })
