import {
  ExpandLessRounded,
  ExpandMoreRounded,
  InboxRounded,
} from '@mui/icons-material'
import {
  alpha,
  Box,
  ListItemText,
  ListItemButton,
  Typography,
  styled,
  Chip,
  Tooltip,
} from '@mui/material'
import { memo, useMemo } from 'react'
import { useTranslation } from 'react-i18next'

import { useIconCache } from '@/hooks/use-icon-cache'
import { formatSmartTopNode, useSmartWeights } from '@/hooks/use-smart-weights'
import { useVerge } from '@/hooks/use-verge'
import { useThemeMode } from '@/services/states'
import type { ResolvedProxyMember } from '@/types/proxy-view'

import { ProxyHead } from './proxy-head'
import { ProxyItem } from './proxy-item'
import { ProxyItemMini } from './proxy-item-mini'
import type { HeadState } from './use-head-state'
import type { IRenderItem } from './use-render-list'

interface RenderProps {
  item: IRenderItem
  stickyed?: boolean
  isChainMode?: boolean
  onLocation: (group: IRenderItem['group']) => void
  onCheckAll: (groupName: string) => void
  onHeadState: (groupName: string, patch: Partial<HeadState>) => void
  onChangeProxy: (
    group: IRenderItem['group'],
    member: ResolvedProxyMember,
  ) => void
  onGroupToggle?: (group: IRenderItem['group']) => void
}

export const ProxyRender = memo(function ProxyRender(props: RenderProps) {
  const { t } = useTranslation()
  const {
    item,
    stickyed: _stickyed = false, // 恢复吸顶阴影时改回 stickyed 并取消下方注释
    onLocation,
    onCheckAll,
    onHeadState,
    onChangeProxy,
    onGroupToggle,
    isChainMode: _ = false,
  } = props
  const { type, group, headState, member, memberCol } = item
  const { verge } = useVerge()
  const enable_group_icon = verge?.enable_group_icon ?? true
  const mode = useThemeMode()
  const isDark = mode === 'dark'
  const itembackgroundcolor = isDark ? '#282A36' : '#ffffff'
  const iconCachePath = useIconCache({
    icon: group.icon,
    cacheKey: group.name.replaceAll(' ', ''),
    enabled: enable_group_icon,
  })

  const showType = headState?.showType

  // Smart 组：头部展示权重最高的节点，节点项展示 rank 徽标
  const isSmartGroup = group.type === 'Smart'
  const groupMemberNames = useMemo(
    () => group.members.map((m) => m.name),
    [group],
  )
  const { topNodes, rankMap } = useSmartWeights(
    group.name,
    isSmartGroup,
    groupMemberNames,
  )

  const memberColItemsMemo = useMemo(() => {
    if (type !== 4 || !memberCol) {
      return null
    }

    return memberCol.map((occurrence) => (
      <ProxyItemMini
        key={`${item.key}-${occurrence.memberIndex}`}
        group={group}
        member={occurrence.member}
        selected={group.now === occurrence.member.ref.name}
        showType={showType}
        smartRank={rankMap.get(occurrence.member.ref.name)}
        onClick={(nextMember) => onChangeProxy(group, nextMember)}
      />
    ))
  }, [type, memberCol, item.key, group, showType, rankMap, onChangeProxy])

  if (type === 0) {
    return (
      <div style={{ padding: '4px 8px' }}>
        <ListItemButton
          dense
          sx={
            {
              // 上游的吸顶阴影；要悬浮感：解构处 _stickyed 改回 stickyed 后取消下面注释
              // boxShadow:
              //   stickyed && headState?.open
              //     ? '0 4px 8px rgba(0, 0, 0, 0.2) !important'
              //     : undefined,
            }
          }
          style={{
            background: itembackgroundcolor,
            height: '100%',
            borderRadius: '8px',
          }}
          onClick={() => {
            if (headState?.open) {
              onGroupToggle?.(group)
            }
            onHeadState?.(group.name, { open: !headState?.open })
          }}
        >
          {enable_group_icon && group.icon?.trim().startsWith('http') && (
            <img
              src={iconCachePath === '' ? group.icon : iconCachePath}
              alt="group icon"
              width="32px"
              style={{ marginRight: '12px', borderRadius: '6px' }}
            />
          )}
          {enable_group_icon && group.icon?.trim().startsWith('data') && (
            <img
              src={group.icon}
              alt="group icon"
              width="32px"
              style={{ marginRight: '12px', borderRadius: '6px' }}
            />
          )}
          {enable_group_icon && group.icon?.trim().startsWith('<svg') && (
            <img
              src={`data:image/svg+xml;base64,${btoa(group.icon)}`}
              alt="group icon"
              width="32px"
            />
          )}
          <ListItemText
            primary={<StyledPrimary>{group.name}</StyledPrimary>}
            secondary={
              <Box
                sx={{
                  display: 'flex',
                  alignItems: 'center',
                  pt: '2px',
                  overflow: 'hidden',
                  whiteSpace: 'nowrap',
                }}
              >
                <Box
                  component="span"
                  sx={{
                    marginTop: '2px',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                  }}
                >
                  <StyledTypeBox>
                    {isSmartGroup ? 'Smart Group' : group.type}
                  </StyledTypeBox>
                  {isSmartGroup
                    ? topNodes.length > 0 && (
                        <StyledSubtitle sx={{ color: 'text.secondary' }}>
                          {topNodes.map(formatSmartTopNode).join(' · ')}
                        </StyledSubtitle>
                      )
                    : group.now && (
                        <StyledSubtitle sx={{ color: 'text.secondary' }}>
                          {group.now}
                        </StyledSubtitle>
                      )}
                </Box>
              </Box>
            }
            slotProps={{
              secondary: {
                component: 'div',
                sx: {
                  display: 'flex',
                  alignItems: 'center',
                  color: '#ccc',
                },
              },
            }}
          />
          <Box sx={{ display: 'flex', alignItems: 'center' }}>
            <Tooltip title={t('proxies.page.labels.proxyCount')} arrow>
              <Chip
                size="small"
                label={`${group.members.length}`}
                sx={{
                  mr: 1,
                  backgroundColor: (theme) =>
                    alpha(theme.palette.primary.main, 0.1),
                  color: (theme) => theme.palette.primary.main,
                }}
              />
            </Tooltip>
            {headState?.open ? <ExpandLessRounded /> : <ExpandMoreRounded />}
          </Box>
        </ListItemButton>
      </div>
    )
  }

  if (type === 1) {
    return (
      <ProxyHead
        sx={{ pl: 2, pr: 3, mt: 0.5, mb: 1 }}
        url={group.testUrl}
        groupName={group.name}
        headState={headState!}
        onLocation={() => onLocation(group)}
        onCheckDelay={() => onCheckAll(group.name)}
        onHeadState={(p) => onHeadState(group.name, p)}
      />
    )
  }

  if (type === 2) {
    return (
      <ProxyItem
        group={group}
        member={member!.member}
        selected={group.now === member?.member.ref.name}
        showType={headState?.showType}
        smartRank={rankMap.get(member?.member.ref.name ?? '')}
        sx={{ py: 0, pl: 2 }}
        onClick={(nextMember) => onChangeProxy(group, nextMember)}
      />
    )
  }

  if (type === 3) {
    return (
      <Box
        sx={{
          py: 2,
          pl: 0,
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        <InboxRounded sx={{ fontSize: '2.5em', color: 'inherit' }} />
        <Typography sx={{ color: 'inherit' }}>
          {t('proxies.page.empty.noProxies')}
        </Typography>
      </Box>
    )
  }

  if (type === 4) {
    return (
      <Box
        sx={{
          height: 56,
          display: 'grid',
          my: 0.5,
          gap: 1,
          px: 2,
          gridTemplateColumns: `repeat(${item.col! || 2}, 1fr)`,
        }}
      >
        {memberColItemsMemo}
      </Box>
    )
  }

  return null
})

const StyledPrimary = styled('span')`
  font-size: 16px;
  font-weight: 700;
  line-height: 1.5;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
`
const StyledSubtitle = styled('span')`
  font-size: 13px;
  overflow: hidden;
  color: text.secondary;
  text-overflow: ellipsis;
  white-space: nowrap;
`

const StyledTypeBox = styled(Box)(({ theme }) => ({
  display: 'inline-block',
  border: '1px solid #ccc',
  borderColor: alpha(theme.palette.primary.main, 0.5),
  color: alpha(theme.palette.primary.main, 0.8),
  borderRadius: 4,
  fontSize: 10,
  padding: '0 4px',
  lineHeight: 1.5,
  marginRight: '8px',
}))
