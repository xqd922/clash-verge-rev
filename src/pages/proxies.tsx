import { Box, Button, ButtonGroup } from '@mui/material'
import { useLockFn } from 'ahooks'
import { useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { closeAllConnections } from 'tauri-plugin-mihomo-api'

import { BasePage } from '@/components/base'
import { ProviderButton } from '@/components/proxy/provider-button'
import { ProxyGroups } from '@/components/proxy/proxy-groups'
import { useVerge } from '@/hooks/use-verge'
import {
  useAppRefreshers,
  useClashConfigData,
} from '@/providers/app-data-context'
import { patchClashMode } from '@/services/cmds'
import { showNotice } from '@/services/notice-service'

const MODES = ['rule', 'global', 'direct'] as const
type Mode = (typeof MODES)[number]
const MODE_SET = new Set<string>(MODES)
const isMode = (value: unknown): value is Mode =>
  typeof value === 'string' && MODE_SET.has(value)

const ProxyPage = () => {
  const { t } = useTranslation()

  const { clashConfig } = useClashConfigData()
  const { refreshClashConfig, refreshProxy } = useAppRefreshers()
  const { verge } = useVerge()

  const normalizedMode = clashConfig?.mode?.toLowerCase()
  const curMode = isMode(normalizedMode) ? normalizedMode : undefined

  const onChangeMode = useLockFn(async (mode: Mode) => {
    // 断开连接
    if (mode !== curMode && verge?.auto_close_connection) {
      closeAllConnections()
    }
    try {
      await patchClashMode(mode)
    } catch (error) {
      console.error('[ProxyDiagnostics] patchClashMode:failed', error)
      showNotice.error('Failed to switch proxy mode:', error)
      return
    }

    const refreshResults = await Promise.allSettled([
      refreshClashConfig(),
      refreshProxy(),
    ])
    if (refreshResults.some((result) => result.status === 'rejected')) {
      console.warn('[ProxyDiagnostics] patchClashMode:refresh-failed')
    }
  })

  useEffect(() => {
    if (normalizedMode && !isMode(normalizedMode)) {
      onChangeMode('rule')
    }
  }, [normalizedMode, onChangeMode])

  return (
    <BasePage
      full
      contentStyle={{ height: '100%' }}
      title={t('proxies.page.title.default')}
      header={
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
          <ProviderButton />

          <ButtonGroup size="small">
            {MODES.map((mode) => (
              <Button
                key={mode}
                variant={mode === curMode ? 'contained' : 'outlined'}
                onClick={() => onChangeMode(mode)}
                sx={{ textTransform: 'capitalize' }}
              >
                {t(`proxies.page.modes.${mode}`)}
              </Button>
            ))}
          </ButtonGroup>
        </Box>
      }
    >
      <ProxyGroups mode={curMode ?? 'rule'} />
    </BasePage>
  )
}

export default ProxyPage
