/**
 * SWR Cache Provider with LRU eviction.
 *
 * The default SWR cache is an unbounded Map that never evicts entries.
 * With date-based subscription keys and icon caches, stale entries
 * accumulate over hours/days and eventually cause WebView2 OOM.
 *
 * This provider wraps a Map and enforces a maximum size via LRU eviction.
 * "Protected" key prefixes (active subscriptions, core config) are never evicted.
 */

const MAX_CACHE_SIZE = 200;
const EVICTION_BATCH = 50;

/** Prefixes that should never be evicted by LRU */
const PROTECTED_PREFIXES = [
  "getClashConfig",
  "getClashInfo",
  "getVersion",
  "getVergeConfig",
  "getProfiles",
  "getAutotemProxy",
  "getRuntimeConfig",
  "getRuntimeExists",
] as const;

function isProtected(key: string): boolean {
  for (const prefix of PROTECTED_PREFIXES) {
    if (key === prefix || key.startsWith(`${prefix}-`)) return true;
  }
  // Protect active subscription keys (the current $sub$ keys are managed by SWR internally)
  if (key.startsWith("$sub$")) return true;
  return false;
}

export function swrCacheProvider(): Map<string, any> {
  const map = new Map<string, any>();
  const accessOrder: Map<string, number> = new Map();
  let accessCounter = 0;

  const evict = () => {
    if (map.size <= MAX_CACHE_SIZE) return;

    // Collect evictable entries sorted by access time (oldest first)
    const evictable: [string, number][] = [];
    for (const [key, order] of accessOrder) {
      if (!isProtected(key) && map.has(key)) {
        evictable.push([key, order]);
      }
    }
    evictable.sort((a, b) => a[1] - b[1]);

    const toRemove = Math.min(evictable.length, EVICTION_BATCH);
    for (let i = 0; i < toRemove; i++) {
      const key = evictable[i][0];
      map.delete(key);
      accessOrder.delete(key);
    }
  };

  return new Proxy(map, {
    get(target, prop, receiver) {
      if (prop === "get") {
        return (key: string) => {
          const val = target.get(key);
          if (val !== undefined) {
            accessOrder.set(key, ++accessCounter);
          }
          return val;
        };
      }
      if (prop === "set") {
        return (key: string, value: any) => {
          target.set(key, value);
          accessOrder.set(key, ++accessCounter);
          // Evict after adding, not before, to keep logic simple
          if (target.size > MAX_CACHE_SIZE + EVICTION_BATCH) {
            evict();
          }
          return target;
        };
      }
      if (prop === "delete") {
        return (key: string) => {
          accessOrder.delete(key);
          return target.delete(key);
        };
      }
      return Reflect.get(target, prop, receiver);
    },
  });
}
