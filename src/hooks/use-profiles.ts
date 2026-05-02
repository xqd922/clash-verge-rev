import useSWR, { mutate } from "swr";
import {
  getProfiles,
  patchProfile,
  patchProfilesConfig,
} from "@/services/cmds";
import { getProxies, updateProxy } from "@/services/api";

export const useProfiles = () => {
  const { data: profiles, mutate: mutateProfiles } = useSWR(
    "getProfiles",
    getProfiles
  );

  const patchProfiles = async (value: Partial<IProfilesConfig>) => {
    await patchProfilesConfig(value);
    mutateProfiles();
  };

  const patchCurrent = async (value: Partial<IProfileItem>) => {
    if (profiles?.current) {
      await patchProfile(profiles.current, value);
      mutateProfiles();
    }
  };

  // 根据selected的节点选择
  const activateSelected = async () => {
    // mihomo reload 后 group 树偶尔有几十毫秒空窗，重试一次容忍掉
    let proxiesData = await getProxies();
    for (let i = 0; i < 2 && !proxiesData?.groups?.length; i++) {
      await new Promise((r) => setTimeout(r, 150));
      proxiesData = await getProxies();
    }
    const profileData = await getProfiles();

    if (!profileData || !proxiesData) return;

    const current = profileData.items?.find(
      (e) => e && e.uid === profileData.current
    );

    if (!current) return;

    // init selected array
    const { selected = [] } = current;
    const selectedMap = Object.fromEntries(
      selected.map((each) => [each.name!, each.now!])
    );

    const pendingUpdates: Promise<unknown>[] = [];
    const newSelected: typeof selected = [];
    const { global, groups } = proxiesData;

    [global, ...groups].forEach(({ type, name, now }) => {
      if (!now || type !== "Selector") return;
      if (selectedMap[name] != null && selectedMap[name] !== now) {
        pendingUpdates.push(updateProxy(name, selectedMap[name]));
      }
      newSelected.push({ name, now: selectedMap[name] });
    });

    if (pendingUpdates.length > 0) {
      // 等所有 selector 偏好都恢复完，再持久化 selected 状态
      await Promise.all(pendingUpdates);
      await patchProfile(profileData.current!, { selected: newSelected });
      mutate("getProxies", getProxies());
    }
  };

  return {
    profiles,
    current: profiles?.items?.find((p) => p && p.uid === profiles.current),
    activateSelected,
    patchProfiles,
    patchCurrent,
    mutateProfiles,
  };
};
