import {
  DeleteForeverRounded,
  PauseCircleOutlineRounded,
  PlayCircleOutlineRounded,
  SettingsRounded,
  WarningRounded,
} from '@mui/icons-material'
import { Box, Typography, alpha, useTheme } from '@mui/material'
import { useLockFn } from 'ahooks'
import { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { type DialogRef, Switch, TooltipIcon } from '@/components/base'
import { SysproxyViewer } from '@/components/setting/mods/sysproxy-viewer'
import { TunViewer } from '@/components/setting/mods/tun-viewer'
import { useServiceUninstaller } from '@/hooks/use-service-uninstaller'
import { useSystemProxyState } from '@/hooks/use-system-proxy-state'
import { useSystemState } from '@/hooks/use-system-state'
import { useVerge } from '@/hooks/use-verge'
import { getRuntimeState, installService, restartCore } from '@/services/cmds'
import { showNotice } from '@/services/notice-service'
import { isAuthorizationCancelled } from '@/utils/is-authorization-cancelled'

interface ProxySwitchProps {
  label?: string
  onError?: (err: Error) => void
  noRightPadding?: boolean
}

interface SwitchRowProps {
  label: string
  active: boolean
  disabled?: boolean
  infoTitle: string
  onInfoClick?: () => void
  extraIcons?: React.ReactNode
  /** Return false to roll back without reporting an error. */
  onToggle: (value: boolean) => Promise<boolean | void>
  onError?: (err: Error) => void
  highlight?: boolean
  /** Keep the previous position until the toggle succeeds. */
  settleOnSuccess?: boolean
}

/**
 * 抽取的子组件：统一的开关 UI
 * active = 真实状态OS/配置 乐观更新
 */
const SwitchRow = ({
  label,
  active,
  disabled,
  infoTitle,
  onInfoClick,
  extraIcons,
  onToggle,
  onError,
  highlight,
  settleOnSuccess = false,
}: SwitchRowProps) => {
  const theme = useTheme()
  const [checked, setChecked] = useState(active)
  const pendingRef = useRef(false)

  if (pendingRef.current) {
    if (!settleOnSuccess && active === checked) pendingRef.current = false
  } else if (checked !== active) {
    setChecked(active)
  }

  const handleChange = (_: React.ChangeEvent, value: boolean) => {
    pendingRef.current = true
    if (!settleOnSuccess) setChecked(value)
    onToggle(value)
      .then((applied) => {
        if (applied === false) {
          setChecked(active)
          return
        }
        if (settleOnSuccess) setChecked(value)
      })
      .catch((err: any) => {
        setChecked(active)
        onError?.(err)
      })
      .finally(() => {
        pendingRef.current = false
      })
  }

  return (
    <Box
      sx={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        p: 1,
        pr: 2,
        borderRadius: 1.5,
        bgcolor: highlight
          ? alpha(theme.palette.success.main, 0.07)
          : 'transparent',
        opacity: disabled ? 0.6 : 1,
        transition: 'background-color 0.3s',
      }}
    >
      <Box sx={{ display: 'flex', alignItems: 'center' }}>
        {active ? (
          <PlayCircleOutlineRounded sx={{ color: 'success.main', mr: 1 }} />
        ) : (
          <PauseCircleOutlineRounded sx={{ color: 'text.disabled', mr: 1 }} />
        )}
        <Typography
          variant="subtitle1"
          sx={{ fontWeight: 500, fontSize: '15px' }}
        >
          {label}
        </Typography>
        <TooltipIcon
          title={infoTitle}
          icon={SettingsRounded}
          onClick={onInfoClick}
          sx={{ ml: 1 }}
        />
        {extraIcons}
      </Box>

      <Switch
        edge="end"
        disabled={disabled}
        checked={checked}
        onChange={handleChange}
      />
    </Box>
  )
}

const ProxyControlSwitches = ({
  label,
  onError,
  noRightPadding = false,
}: ProxySwitchProps) => {
  const { t } = useTranslation()
  const { verge, mutateVerge, patchVerge } = useVerge()
  const { uninstallServiceAndStartSidecar } = useServiceUninstaller()
  const { indicator: systemProxyIndicator, toggleSystemProxy } =
    useSystemProxyState()
  const { runState, isTunModeAvailable, isLoading, mutateSystemState } =
    useSystemState()
  // Offer to uninstall only a service that is actually there and working.
  const isServiceInstallReady = runState.serviceUsable

  const sysproxyRef = useRef<DialogRef>(null)
  const tunRef = useRef<DialogRef>(null)

  const { enable_tun_mode } = verge ?? {}

  // Enabling needs a running core; disabling only writes OS state and must stay available.
  const handleSystemProxyToggle = async (value: boolean) => {
    if (value && !isLoading && runState.mode === 'NotRunning') {
      showNotice.error('settings.feedback.errors.sysproxy.coreNotReady')
      return false
    }
    await toggleSystemProxy(value)
  }

  const writeTunMode = async (value: boolean) => {
    mutateVerge((current) => ({ ...current, enable_tun_mode: value }), false)
    await patchVerge({ enable_tun_mode: value })
  }

  const coreCanUseTun = async () => {
    const refreshed = await mutateSystemState()
    const next = refreshed.data ?? (await getRuntimeState())
    return next.mode === 'Service' || next.tunCapable
  }

  const enableTunAfterCoreReady = async () => {
    if (!(await coreCanUseTun())) {
      showNotice.error(
        'settings.sections.system.notifications.tunMode.enableFailed',
      )
      return false
    }
    await writeTunMode(true)
    showNotice.success('settings.sections.system.notifications.tunMode.enabled')
    return true
  }

  const handleTunToggle = useLockFn(async (value: boolean) => {
    if (!value) {
      await writeTunMode(false)
      return
    }
    // A mismatched service stays in Settings. This switch does not install it.
    if (runState.service === 'versionMismatch') return false

    try {
      if (runState.serviceUsable && runState.mode !== 'Service') {
        await restartCore()
        return await enableTunAfterCoreReady()
      }
      if (!isTunModeAvailable) {
        await installService()
        await restartCore()
        return await enableTunAfterCoreReady()
      }
      await writeTunMode(true)
    } catch (error) {
      if (isAuthorizationCancelled(error)) {
        showNotice.warning(
          'settings.sections.system.notifications.tunMode.unauthorized',
        )
        return false
      }
      showNotice.error(error)
      return false
    }
  })

  const onUninstallService = useLockFn(async () => {
    try {
      await uninstallServiceAndStartSidecar()
    } catch (err) {
      showNotice.error(err)
    }
  })

  const isSystemProxyMode =
    label === t('settings.sections.system.toggles.systemProxy') || !label
  const isTunMode = label === t('settings.sections.system.toggles.tunMode')

  return (
    <Box sx={{ width: '100%', pr: noRightPadding ? 1 : 2 }}>
      {isSystemProxyMode && (
        <SwitchRow
          label={t('settings.sections.proxyControl.fields.systemProxy')}
          active={systemProxyIndicator}
          infoTitle={t('settings.sections.proxyControl.tooltips.systemProxy')}
          onInfoClick={() => sysproxyRef.current?.open()}
          onToggle={handleSystemProxyToggle}
          onError={onError}
          highlight={systemProxyIndicator}
        />
      )}

      {isTunMode && (
        <SwitchRow
          label={t('settings.sections.proxyControl.fields.tunMode')}
          active={(enable_tun_mode && isTunModeAvailable) || false}
          infoTitle={t('settings.sections.proxyControl.tooltips.tunMode')}
          onInfoClick={() => tunRef.current?.open()}
          onToggle={handleTunToggle}
          onError={onError}
          settleOnSuccess
          highlight={(enable_tun_mode && isTunModeAvailable) || false}
          extraIcons={
            <>
              {!isTunModeAvailable && (
                <TooltipIcon
                  title={t(
                    'settings.sections.proxyControl.tooltips.tunUnavailable',
                  )}
                  icon={WarningRounded}
                  sx={{ color: 'warning.main', ml: 1 }}
                />
              )}
              {isServiceInstallReady && (
                <TooltipIcon
                  title={t(
                    'settings.sections.proxyControl.actions.uninstallService',
                  )}
                  icon={DeleteForeverRounded}
                  color="secondary"
                  onClick={onUninstallService}
                  sx={{ ml: 1 }}
                />
              )}
            </>
          }
        />
      )}

      <SysproxyViewer ref={sysproxyRef} />
      <TunViewer ref={tunRef} />
    </Box>
  )
}

export default ProxyControlSwitches
