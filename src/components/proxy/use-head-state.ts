import { useCallback, useEffect, useState } from "react";
import { ProxySortType } from "./use-filter-sort";
import { useProfiles } from "@/hooks/use-profiles";

export interface HeadState {
  open?: boolean;
  showType: boolean;
  sortType: ProxySortType;
  filterText: string;
  textState: "url" | "filter" | null;
  testUrl: string;
}

type HeadStateStorage = Record<string, Record<string, HeadState>>;

const HEAD_STATE_KEY = "proxy-head-state";
const LAST_PROFILE_KEY = "proxy-head-last-profile";
export const DEFAULT_STATE: HeadState = {
  open: false,
  showType: true,
  sortType: 0,
  filterText: "",
  textState: null,
  testUrl: "",
};

function readHeadStateFor(profileId: string): Record<string, HeadState> {
  try {
    const data = JSON.parse(
      localStorage.getItem(HEAD_STATE_KEY) || "{}"
    ) as HeadStateStorage;
    const value = data[profileId];
    if (value && typeof value === "object") return value;
  } catch {}
  return {};
}

export function useHeadStateNew() {
  const { profiles } = useProfiles();
  const current = profiles?.current || "";

  // 同步从 localStorage 读取上次 profile 的状态作为初值，
  // 避免首帧用空对象渲染、随后 effect 恢复时 renderList 长度跳变导致 Virtuoso 抖动。
  const [state, setState] = useState<Record<string, HeadState>>(() => {
    try {
      const lastProfile = localStorage.getItem(LAST_PROFILE_KEY);
      return lastProfile ? readHeadStateFor(lastProfile) : {};
    } catch {
      return {};
    }
  });

  useEffect(() => {
    if (!current) return;
    try {
      localStorage.setItem(LAST_PROFILE_KEY, current);
    } catch {}
    setState(readHeadStateFor(current));
  }, [current]);

  const setHeadState = useCallback(
    (groupName: string, obj: Partial<HeadState>) => {
      setState((old) => {
        const state = old[groupName] || DEFAULT_STATE;
        const ret = { ...old, [groupName]: { ...state, ...obj } };

        // 保存到存储中
        setTimeout(() => {
          try {
            const item = localStorage.getItem(HEAD_STATE_KEY);

            let data = (item ? JSON.parse(item) : {}) as HeadStateStorage;

            if (!data || typeof data !== "object") data = {};

            data[current] = ret;

            localStorage.setItem(HEAD_STATE_KEY, JSON.stringify(data));
          } catch {}
        });

        return ret;
      });
    },
    [current]
  );

  return [state, setHeadState] as const;
}
