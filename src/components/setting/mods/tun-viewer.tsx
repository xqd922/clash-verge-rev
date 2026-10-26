import {
  Box,
  Button,
  FormControl,
  List,
  ListItem,
  ListItemText,
  MenuItem,
  Select,
  TextField,
  Typography,
} from '@mui/material'
import { useLockFn } from 'ahooks'
import type { Ref } from 'react'
import { useImperativeHandle, useState } from 'react'
import { useTranslation } from 'react-i18next'

import {
  BaseDialog,
  BaseSplitChipEditor,
  TooltipIcon,
  DialogRef,
  Switch,
} from '@/components/base'
import { useClash } from '@/hooks/use-clash'
import { enhanceProfiles } from '@/services/cmds'
import { showNotice } from '@/services/notice-service'
import getSystem from '@/utils/get-system'
import { areValidIpCidrs } from '@/utils/network'

import { StackModeSwitch } from './stack-mode-switch'

const OS = getSystem()

const splitList = (value: string) =>
  value
    .split(/[,\n;\r]+/)
    .map((item) => item.trim())
    .filter(Boolean)

const DNS_MODES = ['', 'disabled', 'native', 'hijack'] as const
const CONGESTION_CONTROLLERS = ['cubic', 'reno', 'bbr', 'bbr3'] as const

type TunDnsMode = (typeof DNS_MODES)[number]
type TunCongestionController = '' | (typeof CONGESTION_CONTROLLERS)[number]

interface TunFormValues {
  stack: string
  device: string
  autoRoute: boolean
  routeExcludeAddress: string
  autoRedirect: boolean
  autoRedirectFallbackRuleIndex: number
  autoDetectInterface: boolean
  dnsMode: TunDnsMode
  dnsHijack: string[]
  strictRoute: boolean
  mtu: number
  icmpTimeout: number
  disableIcmpForwarding: boolean
  congestionController: TunCongestionController
  processorsPerChannel: number
  includeMacAddress: string
  excludeMacAddress: string
}

const normalizeDnsMode = (value?: string): TunDnsMode =>
  DNS_MODES.some((mode) => mode === value) ? (value as TunDnsMode) : ''

const normalizeCongestionController = (
  value?: string,
): TunCongestionController =>
  CONGESTION_CONTROLLERS.some((controller) => controller === value)
    ? (value as TunCongestionController)
    : ''

export function TunViewer({ ref }: { ref?: Ref<DialogRef> }) {
  const { t } = useTranslation()

  const { clash, mutateClash, patchClash } = useClash()

  const [open, setOpen] = useState(false)
  const [values, setValues] = useState<TunFormValues>({
    stack: 'mips',
    device: OS === 'macos' ? 'utun1024' : 'Mihomo',
    autoRoute: true,
    routeExcludeAddress: '',
    autoRedirect: false,
    autoRedirectFallbackRuleIndex: 0,
    autoDetectInterface: true,
    dnsMode: '',
    dnsHijack: ['any:53'],
    strictRoute: false,
    mtu: 1500,
    icmpTimeout: 0,
    disableIcmpForwarding: false,
    congestionController: '',
    processorsPerChannel: 1,
    includeMacAddress: '',
    excludeMacAddress: '',
  })

  const routeExcludeAddressItems = splitList(values.routeExcludeAddress)
  const routeExcludeAddressError =
    values.autoRoute &&
    routeExcludeAddressItems.length > 0 &&
    !areValidIpCidrs(routeExcludeAddressItems)
  const routeExcludeAddressHelperText = routeExcludeAddressError
    ? t('settings.modals.tun.messages.invalidRouteExcludeAddress')
    : t('settings.modals.tun.messages.routeExcludeAddressHint')

  useImperativeHandle(ref, () => ({
    open: () => {
      setOpen(true)
      const nextAutoRoute = clash?.tun['auto-route'] ?? true
      const rawAutoRedirect = clash?.tun['auto-redirect'] ?? false
      const computedAutoRedirect =
        OS === 'linux' ? (nextAutoRoute ? rawAutoRedirect : false) : false
      setValues({
        stack: clash?.tun.stack ?? 'mips',
        device: clash?.tun.device ?? (OS === 'macos' ? 'utun1024' : 'Mihomo'),
        autoRoute: nextAutoRoute,
        routeExcludeAddress: (clash?.tun['route-exclude-address'] ?? []).join(
          ',',
        ),
        autoRedirect: computedAutoRedirect,
        autoRedirectFallbackRuleIndex:
          clash?.tun['auto-redirect-iproute2-fallback-rule-index'] ?? 0,
        autoDetectInterface: clash?.tun['auto-detect-interface'] ?? true,
        dnsMode: normalizeDnsMode(clash?.tun['dns-mode']),
        dnsHijack: clash?.tun['dns-hijack'] ?? ['any:53'],
        strictRoute: clash?.tun['strict-route'] ?? false,
        mtu: clash?.tun.mtu ?? 1500,
        icmpTimeout: clash?.tun['icmp-timeout'] ?? 0,
        disableIcmpForwarding: clash?.tun['disable-icmp-forwarding'] ?? false,
        congestionController: normalizeCongestionController(
          clash?.tun['congestion-controller'],
        ),
        processorsPerChannel: clash?.tun['processors-per-channel'] ?? 1,
        includeMacAddress: (clash?.tun['include-mac-address'] ?? []).join(','),
        excludeMacAddress: (clash?.tun['exclude-mac-address'] ?? []).join(','),
      })
    },
    close: () => setOpen(false),
  }))

  const onSave = useLockFn(async () => {
    try {
      const routeExcludeAddress = routeExcludeAddressItems

      if (routeExcludeAddressError) {
        showNotice.error(
          'settings.modals.tun.messages.invalidRouteExcludeAddress',
        )
        return
      }

      const tun: IConfigData['tun'] = {
        stack: values.stack,
        device:
          values.device === ''
            ? OS === 'macos'
              ? 'utun1024'
              : 'Mihomo'
            : values.device,
        'auto-route': values.autoRoute,
        'route-exclude-address': routeExcludeAddress,
        ...(OS === 'linux'
          ? {
              'auto-redirect': values.autoRedirect,
              'auto-redirect-iproute2-fallback-rule-index':
                values.autoRedirectFallbackRuleIndex,
            }
          : {}),
        'dns-mode': values.dnsMode,
        'auto-detect-interface': values.autoDetectInterface,
        'dns-hijack': values.dnsHijack[0] === '' ? [] : values.dnsHijack,
        'strict-route': values.strictRoute,
        'icmp-timeout': values.icmpTimeout,
        'disable-icmp-forwarding': values.disableIcmpForwarding,
        'include-mac-address': splitList(values.includeMacAddress),
        'exclude-mac-address': splitList(values.excludeMacAddress),
        ...(values.stack.toLowerCase() === 'mips'
          ? {
              'congestion-controller': values.congestionController,
            }
          : {}),
        ...(values.stack.toLowerCase() === 'gvisor'
          ? {
              'processors-per-channel': values.processorsPerChannel,
            }
          : {}),
        mtu: values.mtu ?? 1500,
      }
      await patchClash({ tun })
      await mutateClash(
        (old) => ({
          ...old!,
          tun,
        }),
        false,
      )
      setOpen(false)
      showNotice.success('settings.modals.tun.messages.applied')
      void enhanceProfiles().catch((err: any) => {
        showNotice.error(err)
      })
    } catch (err: any) {
      showNotice.error(err)
    }
  })

  return (
    <BaseDialog
      open={open}
      title={
        <Box sx={{ display: 'flex', justifyContent: 'space-between', gap: 1 }}>
          <Typography variant="h6">{t('settings.modals.tun.title')}</Typography>
          <Button
            variant="outlined"
            size="small"
            onClick={async () => {
              const tun: IConfigData['tun'] = {
                stack: 'mips',
                device: OS === 'macos' ? 'utun1024' : 'Mihomo',
                'auto-route': true,
                ...(OS === 'linux'
                  ? {
                      'auto-redirect': false,
                      'auto-redirect-iproute2-fallback-rule-index': 0,
                    }
                  : {}),
                'dns-mode': '',
                'auto-detect-interface': true,
                'dns-hijack': ['any:53'],
                'route-exclude-address': [],
                'strict-route': false,
                'icmp-timeout': 0,
                'disable-icmp-forwarding': false,
                'include-mac-address': [],
                'exclude-mac-address': [],
                mtu: 1500,
              }
              setValues({
                stack: 'mips',
                device: OS === 'macos' ? 'utun1024' : 'Mihomo',
                autoRoute: true,
                routeExcludeAddress: '',
                autoRedirect: false,
                autoRedirectFallbackRuleIndex: 0,
                autoDetectInterface: true,
                dnsMode: '',
                dnsHijack: ['any:53'],
                strictRoute: false,
                mtu: 1500,
                icmpTimeout: 0,
                disableIcmpForwarding: false,
                congestionController: '',
                processorsPerChannel: 1,
                includeMacAddress: '',
                excludeMacAddress: '',
              })
              await patchClash({ tun })
              await mutateClash(
                (old) => ({
                  ...old!,
                  tun,
                }),
                false,
              )
            }}
          >
            {t('shared.actions.resetToDefault')}
          </Button>
        </Box>
      }
      contentSx={{ width: 450 }}
      okBtn={t('shared.actions.save')}
      cancelBtn={t('shared.actions.cancel')}
      onClose={() => setOpen(false)}
      onCancel={() => setOpen(false)}
      onOk={onSave}
    >
      <List>
        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText primary={t('settings.modals.tun.fields.stack')} />
          <StackModeSwitch
            value={values.stack}
            onChange={(value) => {
              setValues((v) => ({
                ...v,
                stack: value,
              }))
            }}
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText primary={t('settings.modals.tun.fields.device')} />
          <TextField
            autoComplete="new-password"
            size="small"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck="false"
            sx={{ width: 250 }}
            value={values.device}
            placeholder="Mihomo"
            onChange={(e) =>
              setValues((v) => ({ ...v, device: e.target.value }))
            }
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText primary={t('settings.modals.tun.fields.autoRoute')} />
          <Switch
            edge="end"
            checked={values.autoRoute}
            onChange={(_, c) =>
              setValues((v) => ({
                ...v,
                autoRoute: c,
                autoRedirect: c ? v.autoRedirect : false,
              }))
            }
          />
        </ListItem>

        {OS === 'linux' && (
          <>
            <ListItem sx={{ padding: '5px 2px' }}>
              <ListItemText
                primary={t('settings.modals.tun.fields.autoRedirect')}
                sx={{ maxWidth: 'fit-content' }}
              />
              <TooltipIcon
                title={t('settings.modals.tun.tooltips.autoRedirect')}
                sx={{ opacity: values.autoRoute ? 0.7 : 0.3 }}
              />
              <Switch
                edge="end"
                checked={values.autoRedirect}
                onChange={(_, c) =>
                  setValues((v) => ({
                    ...v,
                    autoRedirect: v.autoRoute ? c : v.autoRedirect,
                  }))
                }
                disabled={!values.autoRoute}
                sx={{ marginLeft: 'auto' }}
              />
            </ListItem>

            <ListItem sx={{ padding: '5px 2px' }}>
              <ListItemText
                primary={t(
                  'settings.modals.tun.fields.autoRedirectFallbackRuleIndex',
                )}
              />
              <TextField
                autoComplete="new-password"
                size="small"
                type="number"
                sx={{ width: 250 }}
                value={values.autoRedirectFallbackRuleIndex}
                disabled={!values.autoRedirect}
                placeholder="0"
                onChange={(e) =>
                  setValues((v) => ({
                    ...v,
                    autoRedirectFallbackRuleIndex: Math.max(
                      Number.parseInt(e.target.value, 10) || 0,
                      0,
                    ),
                  }))
                }
              />
            </ListItem>
          </>
        )}

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText primary={t('settings.modals.tun.fields.strictRoute')} />
          <Switch
            edge="end"
            checked={values.strictRoute}
            onChange={(_, c) => setValues((v) => ({ ...v, strictRoute: c }))}
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText
            primary={t('settings.modals.tun.fields.autoDetectInterface')}
          />
          <Switch
            edge="end"
            checked={values.autoDetectInterface}
            onChange={(_, c) =>
              setValues((v) => ({ ...v, autoDetectInterface: c }))
            }
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText
            primary={t('settings.modals.tun.fields.dnsMode')}
            sx={{ maxWidth: 'fit-content' }}
          />
          <TooltipIcon
            title={t('settings.modals.tun.tooltips.dnsMode')}
            sx={{ opacity: 0.7 }}
          />
          <FormControl size="small" sx={{ width: 250, marginLeft: 'auto' }}>
            <Select
              displayEmpty
              value={values.dnsMode}
              onChange={(e) =>
                setValues((v) => ({
                  ...v,
                  dnsMode: normalizeDnsMode(e.target.value),
                }))
              }
            >
              <MenuItem value="">
                {t('settings.modals.tun.options.auto')}
              </MenuItem>
              <MenuItem value="disabled">
                {t('settings.modals.tun.options.disabled')}
              </MenuItem>
              <MenuItem value="native">
                {t('settings.modals.tun.options.native')}
              </MenuItem>
              <MenuItem value="hijack">
                {t('settings.modals.tun.options.hijack')}
              </MenuItem>
            </Select>
          </FormControl>
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText primary={t('settings.modals.tun.fields.dnsHijack')} />
          <TextField
            autoComplete="new-password"
            size="small"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck="false"
            sx={{ width: 250 }}
            value={values.dnsHijack.join(',')}
            placeholder={t('settings.modals.tun.tooltips.dnsHijack')}
            onChange={(e) =>
              setValues((v) => ({ ...v, dnsHijack: e.target.value.split(',') }))
            }
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText primary={t('settings.modals.tun.fields.mtu')} />
          <TextField
            autoComplete="new-password"
            size="small"
            type="number"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck="false"
            sx={{ width: 250 }}
            value={values.mtu}
            placeholder="1500"
            onChange={(e) =>
              setValues((v) => ({
                ...v,
                mtu: parseInt(e.target.value),
              }))
            }
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText
            primary={t('settings.modals.tun.fields.icmpTimeout')}
            sx={{ maxWidth: 'fit-content' }}
          />
          <TooltipIcon
            title={t('settings.modals.tun.tooltips.icmpTimeout')}
            sx={{ opacity: 0.7 }}
          />
          <TextField
            autoComplete="new-password"
            size="small"
            type="number"
            sx={{ width: 250, marginLeft: 'auto' }}
            value={values.icmpTimeout}
            placeholder="0"
            onChange={(e) =>
              setValues((v) => ({
                ...v,
                icmpTimeout: Math.max(
                  Number.parseInt(e.target.value, 10) || 0,
                  0,
                ),
              }))
            }
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText
            primary={t('settings.modals.tun.fields.disableIcmpForwarding')}
            sx={{ maxWidth: 'fit-content' }}
          />
          <TooltipIcon
            title={t('settings.modals.tun.tooltips.disableIcmpForwarding')}
            sx={{ opacity: 0.7 }}
          />
          <Switch
            edge="end"
            checked={values.disableIcmpForwarding}
            onChange={(_, c) =>
              setValues((v) => ({ ...v, disableIcmpForwarding: c }))
            }
            sx={{ marginLeft: 'auto' }}
          />
        </ListItem>

        {values.stack.toLowerCase() === 'mips' && (
          <ListItem sx={{ padding: '5px 2px' }}>
            <ListItemText
              primary={t('settings.modals.tun.fields.congestionController')}
              sx={{ maxWidth: 'fit-content' }}
            />
            <TooltipIcon
              title={t('settings.modals.tun.tooltips.congestionController')}
              sx={{ opacity: 0.7 }}
            />
            <FormControl size="small" sx={{ width: 250, marginLeft: 'auto' }}>
              <Select
                displayEmpty
                value={values.congestionController}
                onChange={(e) =>
                  setValues((v) => ({
                    ...v,
                    congestionController: normalizeCongestionController(
                      e.target.value,
                    ),
                  }))
                }
              >
                <MenuItem value="">
                  {t('settings.modals.tun.options.auto')}
                </MenuItem>
                {CONGESTION_CONTROLLERS.map((controller) => (
                  <MenuItem key={controller} value={controller}>
                    {controller}
                  </MenuItem>
                ))}
              </Select>
            </FormControl>
          </ListItem>
        )}

        {values.stack.toLowerCase() === 'gvisor' && (
          <ListItem sx={{ padding: '5px 2px' }}>
            <ListItemText
              primary={t('settings.modals.tun.fields.processorsPerChannel')}
              sx={{ maxWidth: 'fit-content' }}
            />
            <TooltipIcon
              title={t('settings.modals.tun.tooltips.processorsPerChannel')}
              sx={{ opacity: 0.7 }}
            />
            <TextField
              autoComplete="new-password"
              size="small"
              type="number"
              sx={{ width: 250, marginLeft: 'auto' }}
              value={values.processorsPerChannel}
              placeholder="1"
              onChange={(e) =>
                setValues((v) => ({
                  ...v,
                  processorsPerChannel: Math.max(
                    Number.parseInt(e.target.value, 10) || 1,
                    1,
                  ),
                }))
              }
            />
          </ListItem>
        )}

        <BaseSplitChipEditor
          value={values.routeExcludeAddress}
          placeholder="192.168.0.0/16"
          ariaLabel={t('settings.modals.tun.fields.routeExcludeAddress')}
          disabled={!values.autoRoute}
          error={routeExcludeAddressError}
          helperText={routeExcludeAddressHelperText}
          onChange={(nextValue) =>
            setValues((v) => ({ ...v, routeExcludeAddress: nextValue }))
          }
          renderHeader={(modeToggle) => (
            <ListItem sx={{ padding: '5px 2px' }}>
              <ListItemText
                primary={t('settings.modals.tun.fields.routeExcludeAddress')}
              />
              {modeToggle ? (
                <Box sx={{ marginLeft: 'auto' }}>{modeToggle}</Box>
              ) : null}
            </ListItem>
          )}
        />

        <BaseSplitChipEditor
          value={values.includeMacAddress}
          placeholder="00:11:22:33:44:55"
          ariaLabel={t('settings.modals.tun.fields.includeMacAddress')}
          onChange={(nextValue) =>
            setValues((v) => ({ ...v, includeMacAddress: nextValue }))
          }
          renderHeader={(modeToggle) => (
            <ListItem sx={{ padding: '5px 2px' }}>
              <ListItemText
                primary={t('settings.modals.tun.fields.includeMacAddress')}
                sx={{ maxWidth: 'fit-content' }}
              />
              <TooltipIcon
                title={t('settings.modals.tun.tooltips.macAddress')}
                sx={{ opacity: 0.7 }}
              />
              {modeToggle ? (
                <Box sx={{ marginLeft: 'auto' }}>{modeToggle}</Box>
              ) : null}
            </ListItem>
          )}
        />

        <BaseSplitChipEditor
          value={values.excludeMacAddress}
          placeholder="00:11:22:33:44:55"
          ariaLabel={t('settings.modals.tun.fields.excludeMacAddress')}
          onChange={(nextValue) =>
            setValues((v) => ({ ...v, excludeMacAddress: nextValue }))
          }
          renderHeader={(modeToggle) => (
            <ListItem sx={{ padding: '5px 2px' }}>
              <ListItemText
                primary={t('settings.modals.tun.fields.excludeMacAddress')}
                sx={{ maxWidth: 'fit-content' }}
              />
              <TooltipIcon
                title={t('settings.modals.tun.tooltips.macAddress')}
                sx={{ opacity: 0.7 }}
              />
              {modeToggle ? (
                <Box sx={{ marginLeft: 'auto' }}>{modeToggle}</Box>
              ) : null}
            </ListItem>
          )}
        />
      </List>
    </BaseDialog>
  )
}
