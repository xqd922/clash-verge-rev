import { useLocalStorage } from 'foxact/use-local-storage'
import { type LogLevel } from 'tauri-plugin-mihomo-api'

const defaultClashLog: IClashLog = {
  enable: true,
  logLevel: 'info',
  logFilter: 'all',
  logOrder: 'asc',
}

const KNOWN_LOG_LEVELS: readonly string[] = [
  'debug',
  'info',
  'warning',
  'error',
  'silent',
]

// 旧版本可能往 localStorage 写入过非法等级(如大写 "INFO"、别名 "warn"),
// 内核 /logs 接口对 level 严格校验,非法值会返回 400 导致日志订阅永远失败
export const sanitizeClashLogLevel = (value: unknown): LogLevel => {
  if (typeof value !== 'string') return defaultClashLog.logLevel
  const normalized = value.toLowerCase()
  const candidate = normalized === 'warn' ? 'warning' : normalized
  return KNOWN_LOG_LEVELS.includes(candidate)
    ? (candidate as LogLevel)
    : defaultClashLog.logLevel
}

export const useClashLog = () =>
  useLocalStorage<IClashLog>('clash-log', defaultClashLog, {
    serializer: JSON.stringify,
    deserializer: JSON.parse,
  })
