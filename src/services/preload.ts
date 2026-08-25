import { getRuntimeConfig, getVergeConfig } from './cmds'
import {
  cacheLanguage,
  getCachedLanguage,
  initializeLanguage,
  resolveLanguage,
} from './i18n'

let vergeConfigCache: IVergeConfig | null | undefined

// 与 vergeConfigCache 同理：为运行时 Clash 配置提供同步首帧数据，
// 避免设置页开关（如 IPv6）先渲染默认值、数据到达后再翻转出现动画
let runtimeConfigCache: IConfigData | null | undefined

export const setPreloadRuntimeConfig = (config: IConfigData | null) => {
  runtimeConfigCache = config
}

export const getPreloadRuntimeConfig = () => runtimeConfigCache

export const preloadRuntimeConfig = async () => {
  try {
    const config = await getRuntimeConfig()
    setPreloadRuntimeConfig(config)
    return config
  } catch (error) {
    console.warn('[preload.ts] Failed to read runtime config:', error)
    setPreloadRuntimeConfig(null)
    return null
  }
}

const detectSystemTheme = (): 'light' | 'dark' => {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function')
    return 'light'
  return window.matchMedia('(prefers-color-scheme: dark)').matches
    ? 'dark'
    : 'light'
}

const getThemeModeFromWindow = (): IVergeConfig['theme_mode'] | undefined => {
  if (typeof window === 'undefined') return undefined
  const mode = (
    window as typeof window & {
      __VERGE_INITIAL_THEME_MODE?: unknown
    }
  ).__VERGE_INITIAL_THEME_MODE
  if (mode === 'light' || mode === 'dark' || mode === 'system') {
    return mode
  }
  return undefined
}

export const resolveThemeMode = (
  vergeConfig?: IVergeConfig | null,
): 'light' | 'dark' => {
  const initialMode = vergeConfig?.theme_mode ?? getThemeModeFromWindow()
  if (initialMode === 'dark' || initialMode === 'light') {
    return initialMode
  }
  return detectSystemTheme()
}

export const setPreloadConfig = (config: IVergeConfig | null) => {
  vergeConfigCache = config
}

export const getPreloadConfig = () => vergeConfigCache

export const preloadConfig = async () => {
  try {
    const config = await getVergeConfig()
    setPreloadConfig(config)
    return config
  } catch (error) {
    console.warn('[preload.ts] Failed to read Verge config:', error)
    setPreloadConfig(null)
    return null
  }
}

export const preloadLanguage = async (
  vergeConfig?: IVergeConfig | null,
  loadConfig: () => Promise<IVergeConfig | null> = preloadConfig,
) => {
  const cachedLanguage = getCachedLanguage()
  if (cachedLanguage) {
    return cachedLanguage
  }

  let resolvedConfig = vergeConfig

  if (resolvedConfig === undefined) {
    try {
      resolvedConfig = await loadConfig()
    } catch (error) {
      console.warn(
        '[preload.ts] Failed to read language from Verge config:',
        error,
      )
      resolvedConfig = null
    }
  }

  const languageFromConfig = resolvedConfig?.language
  if (languageFromConfig) {
    const resolved = resolveLanguage(languageFromConfig)
    cacheLanguage(resolved)
    return resolved
  }

  const browserLanguage = resolveLanguage(
    typeof navigator !== 'undefined' ? navigator.language : undefined,
  )
  cacheLanguage(browserLanguage)
  return browserLanguage
}

export const preloadAppData = async () => {
  const configPromise = preloadConfig()
  const runtimeConfigPromise = preloadRuntimeConfig()
  const initialLanguage = await preloadLanguage(undefined, () => configPromise)
  const [config] = await Promise.all([
    configPromise,
    initializeLanguage(initialLanguage),
    runtimeConfigPromise,
  ])
  const initialThemeMode = resolveThemeMode(config)
  return { initialThemeMode }
}
