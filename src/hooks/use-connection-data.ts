import { mutate } from "swr";
import { MihomoWebSocket } from "tauri-plugin-mihomo-api";

import { useMihomoWsSubscription } from "./use-mihomo-ws-subscription";

const MAX_CLOSED_CONNS_NUM = 500;

export const initConnData: ConnectionMonitorData = {
  uploadTotal: 0,
  downloadTotal: 0,
  activeConnections: [],
  closedConnections: [],
};

export interface ConnectionMonitorData {
  uploadTotal: number;
  downloadTotal: number;
  activeConnections: IConnectionsItem[];
  closedConnections: IConnectionsItem[];
}

const trimClosedConnections = (
  closedConnections: IConnectionsItem[],
): IConnectionsItem[] =>
  closedConnections.length > MAX_CLOSED_CONNS_NUM
    ? closedConnections.slice(-MAX_CLOSED_CONNS_NUM)
    : closedConnections;

const mergeConnectionSnapshot = (
  payload: IConnections,
  previous: ConnectionMonitorData = initConnData,
): ConnectionMonitorData => {
  const nextConnections = payload.connections ?? [];
  const previousActive = previous.activeConnections ?? [];

  // Build lookup map once, mutate in place to avoid extra allocations
  const nextById = new Map<string, IConnectionsItem>();
  for (const conn of nextConnections) {
    nextById.set(conn.id, conn);
  }

  // Pre-allocate result array with estimated capacity
  const activeConnections: IConnectionsItem[] = [];
  const newlyClosed: IConnectionsItem[] = [];

  // Process previously active connections
  for (const prev of previousActive) {
    const next = nextById.get(prev.id);
    if (next) {
      // Connection still active - compute speed delta, mutate in place
      next.curUpload = next.upload - prev.upload;
      next.curDownload = next.download - prev.download;
      activeConnections.push(next);
      nextById.delete(prev.id);
    } else {
      // Connection closed
      newlyClosed.push(prev);
    }
  }

  // Add newcomers (remaining in the map are new connections)
  for (const conn of nextById.values()) {
    conn.curUpload = 0;
    conn.curDownload = 0;
    activeConnections.push(conn);
  }

  // Merge closed connections with trimming
  const prevClosed = previous.closedConnections ?? [];
  let closedConnections: IConnectionsItem[];
  if (newlyClosed.length === 0) {
    closedConnections = prevClosed;
  } else {
    const merged =
      prevClosed.length > 0 ? prevClosed.concat(newlyClosed) : newlyClosed;
    closedConnections = trimClosedConnections(merged);
  }

  return {
    uploadTotal: payload.uploadTotal ?? 0,
    downloadTotal: payload.downloadTotal ?? 0,
    activeConnections,
    closedConnections,
  };
};

const CONN_THROTTLE_MS = 200;

export const useConnectionData = () => {
  const { response, refresh, subscriptionCacheKey } =
    useMihomoWsSubscription<ConnectionMonitorData>({
      storageKey: "mihomo_connection_date",
      buildSubscriptKey: (date) => `getClashConnection-${date}`,
      fallbackData: initConnData,
      connect: () => MihomoWebSocket.connect_connections(),
      setupHandlers: ({ next, scheduleReconnect, isMounted }) => {
        let pendingPayload: IConnections | null = null;
        let throttleTimer: ReturnType<typeof setTimeout> | null = null;

        const flush = () => {
          throttleTimer = null;
          if (!pendingPayload || !isMounted()) return;
          const payload = pendingPayload;
          pendingPayload = null;
          next(null, (old = initConnData) =>
            mergeConnectionSnapshot(payload, old),
          );
        };

        return {
          handleMessage: (data) => {
            if (data.startsWith("Websocket error")) {
              next(data);
              void scheduleReconnect();
              return;
            }

            try {
              // Only keep the latest payload; skip intermediate snapshots
              pendingPayload = JSON.parse(data) as IConnections;
              if (!throttleTimer) {
                throttleTimer = setTimeout(flush, CONN_THROTTLE_MS);
              }
            } catch (error) {
              next(error);
            }
          },
          cleanup: () => {
            if (throttleTimer) {
              clearTimeout(throttleTimer);
              throttleTimer = null;
            }
            pendingPayload = null;
          },
        };
      },
    });

  const clearClosedConnections = () => {
    if (!subscriptionCacheKey) return;
    mutate(subscriptionCacheKey, {
      uploadTotal: response.data?.uploadTotal ?? 0,
      downloadTotal: response.data?.downloadTotal ?? 0,
      activeConnections: response.data?.activeConnections ?? [],
      closedConnections: [],
    });
  };

  return {
    response,
    refreshGetClashConnection: refresh,
    clearClosedConnections,
  };
};
