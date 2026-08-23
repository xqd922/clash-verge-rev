import { useCallback, useSyncExternalStore } from 'react'

import delayManager, { type DelaySnapshot } from '@/services/delay'

/** Preserves unchanged group snapshot identities when another group settles. */
export const useGroupsDelays = (
  groups: readonly string[],
): ReadonlyMap<string, DelaySnapshot> => {
  // Subscribe by group membership; callers rebuild the array on every render.
  const groupKey = groups.join(' ')

  const subscribe = useCallback(
    (onSettle: () => void) => {
      const names = groupKey ? groupKey.split(' ') : []
      const unsubscribes = names.map((name) =>
        delayManager.addGroupListener(name, onSettle),
      )
      return () => {
        for (const unsubscribe of unsubscribes) unsubscribe()
      }
    },
    [groupKey],
  )

  const read = useCallback(
    () => delayManager.groupsDelays(groupKey),
    [groupKey],
  )

  return useSyncExternalStore(subscribe, read, read)
}
