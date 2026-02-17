import {
  AutoGraphRounded,
  CachedRounded,
  RestartAltRounded,
  SwitchAccessShortcutRounded,
} from "@mui/icons-material";
import { LoadingButton } from "@mui/lab";
import {
  Box,
  Chip,
  CircularProgress,
  Divider,
  List,
  ListItemButton,
  ListItemText,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import { useLockFn } from "ahooks";
import type { Ref } from "react";
import { useImperativeHandle, useState } from "react";
import { useTranslation } from "react-i18next";
import { mutate } from "swr";
import {
  closeAllConnections,
  flushSmartCache,
  getProxies,
  unfixedProxy,
  upgradeCore,
  upgradeLgbm,
} from "tauri-plugin-mihomo-api";

import { BaseDialog, DialogRef } from "@/components/base";
import { useVerge } from "@/hooks/use-verge";
import { changeClashCore, restartCore } from "@/services/cmds";
import { showNotice } from "@/services/notice-service";

const VALID_CORE = [
  {
    name: "Mihomo",
    core: "verge-mihomo",
    chipKey: "settings.modals.clashCore.variants.release",
  },
  {
    name: "Mihomo Alpha",
    core: "verge-mihomo-alpha",
    chipKey: "settings.modals.clashCore.variants.alpha",
  },
  {
    name: "Mihomo Smart",
    core: "verge-mihomo-smart",
    chipKey: "settings.modals.clashCore.variants.smart",
  },
];

export function ClashCoreViewer({ ref }: { ref?: Ref<DialogRef> }) {
  const { t } = useTranslation();

  const { verge, mutateVerge, patchVerge } = useVerge();

  const [open, setOpen] = useState(false);
  const [upgrading, setUpgrading] = useState(false);
  const [upgradingLgbm, setUpgradingLgbm] = useState(false);
  const [restarting, setRestarting] = useState(false);
  const [flushingCache, setFlushingCache] = useState(false);
  const [changingCore, setChangingCore] = useState<string | null>(null);

  useImperativeHandle(ref, () => ({
    open: () => setOpen(true),
    close: () => setOpen(false),
  }));

  const {
    clash_core = "verge-mihomo",
    lgbm_auto_update,
    lgbm_update_interval,
    lgbm_url,
    smart_collector_size,
  } = verge ?? {};
  const isSmartCore = clash_core === "verge-mihomo-smart";

  const onCoreChange = useLockFn(async (core: string) => {
    if (core === clash_core) return;

    try {
      setChangingCore(core);
      closeAllConnections();
      const errorMsg = await changeClashCore(core);

      if (errorMsg) {
        showNotice.error(errorMsg);
        setChangingCore(null);
        return;
      }

      mutateVerge();
      setTimeout(async () => {
        mutate("getClashConfig");
        mutate("getVersion");
        // After switching to Smart core, unfix all Smart groups
        if (core === "verge-mihomo-smart") {
          try {
            const proxiesData = await getProxies();
            const groups = Object.values(proxiesData.proxies).filter(
              (p) => p?.type === "Smart" && p?.all,
            );
            await Promise.allSettled(groups.map((g) => unfixedProxy(g!.name)));
            mutate("getProxies");
          } catch {
            // ignore — core might still be starting
          }
        }
        setChangingCore(null);
      }, 500);
    } catch (err) {
      setChangingCore(null);
      showNotice.error(err);
    }
  });

  const onRestart = useLockFn(async () => {
    try {
      setRestarting(true);
      await restartCore();
      showNotice.success(
        t("settings.feedback.notifications.clash.restartSuccess"),
      );
      setRestarting(false);
    } catch (err) {
      setRestarting(false);
      showNotice.error(err);
    }
  });

  const onUpgrade = useLockFn(async () => {
    try {
      setUpgrading(true);
      await upgradeCore();
      setUpgrading(false);
      showNotice.success(
        t("settings.feedback.notifications.clash.versionUpdated"),
      );
    } catch (err: any) {
      setUpgrading(false);
      const errMsg = err?.response?.data?.message ?? String(err);
      const showMsg = errMsg.includes("already using latest version")
        ? t("settings.feedback.notifications.clash.alreadyLatestVersion")
        : errMsg;
      showNotice.info(showMsg);
    }
  });

  const onFlushSmartCache = useLockFn(async () => {
    try {
      setFlushingCache(true);
      await flushSmartCache();
      setFlushingCache(false);
      showNotice.success(
        t("settings.feedback.notifications.clash.smartCacheFlushed"),
      );
    } catch (err: any) {
      setFlushingCache(false);
      showNotice.error(err);
    }
  });

  const onUpgradeLgbm = useLockFn(async () => {
    try {
      setUpgradingLgbm(true);
      await upgradeLgbm();
      setUpgradingLgbm(false);
      showNotice.success(
        t("settings.feedback.notifications.clash.lgbmModelUpdated"),
      );
    } catch (err: any) {
      setUpgradingLgbm(false);
      showNotice.error(err);
    }
  });

  return (
    <BaseDialog
      open={open}
      title={
        <Box display="flex" justifyContent="space-between">
          {t("settings.sections.clash.form.fields.clashCore")}
          <Box>
            {isSmartCore && (
              <>
                <LoadingButton
                  variant="contained"
                  size="small"
                  startIcon={<CachedRounded />}
                  loadingPosition="start"
                  loading={flushingCache}
                  disabled={restarting || changingCore !== null || upgrading}
                  sx={{ marginRight: "8px" }}
                  onClick={onFlushSmartCache}
                >
                  {t("settings.modals.clashCore.actions.flushSmartCache")}
                </LoadingButton>
                <LoadingButton
                  variant="contained"
                  size="small"
                  startIcon={<AutoGraphRounded />}
                  loadingPosition="start"
                  loading={upgradingLgbm}
                  disabled={restarting || changingCore !== null || upgrading}
                  sx={{ marginRight: "8px" }}
                  onClick={onUpgradeLgbm}
                >
                  {t("settings.modals.clashCore.actions.upgradeLgbm")}
                </LoadingButton>
              </>
            )}
            <LoadingButton
              variant="contained"
              size="small"
              startIcon={<SwitchAccessShortcutRounded />}
              loadingPosition="start"
              loading={upgrading}
              disabled={restarting || changingCore !== null}
              sx={{ marginRight: "8px" }}
              onClick={onUpgrade}
            >
              {t("shared.actions.upgrade")}
            </LoadingButton>
            <LoadingButton
              variant="contained"
              size="small"
              startIcon={<RestartAltRounded />}
              loadingPosition="start"
              loading={restarting}
              disabled={upgrading}
              onClick={onRestart}
            >
              {t("shared.actions.restart")}
            </LoadingButton>
          </Box>
        </Box>
      }
      contentSx={{
        pb: 0,
        width: isSmartCore ? 480 : 400,
        height: isSmartCore ? "auto" : 230,
        maxHeight: 500,
        overflowY: "auto",
        userSelect: "text",
        marginTop: "-8px",
      }}
      disableOk
      cancelBtn={t("shared.actions.close")}
      onClose={() => setOpen(false)}
      onCancel={() => setOpen(false)}
    >
      <List component="nav">
        {VALID_CORE.map((each) => (
          <ListItemButton
            key={each.core}
            selected={each.core === clash_core}
            onClick={() => onCoreChange(each.core)}
            disabled={changingCore !== null || restarting || upgrading}
          >
            <ListItemText primary={each.name} secondary={`/${each.core}`} />
            {changingCore === each.core ? (
              <CircularProgress size={20} sx={{ mr: 1 }} />
            ) : (
              <Chip label={t(each.chipKey)} size="small" />
            )}
          </ListItemButton>
        ))}
      </List>

      {isSmartCore && (
        <>
          <Divider sx={{ mt: 1 }} />
          <Box sx={{ px: 2, py: 1.5 }}>
            <Typography variant="subtitle2" sx={{ mb: 1.5 }}>
              {t("settings.modals.clashCore.smartConfig.title")}
            </Typography>
            <Box
              sx={{
                display: "flex",
                flexDirection: "column",
                gap: 1.5,
              }}
            >
              <Box
                sx={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                }}
              >
                <Typography variant="body2">
                  {t("settings.modals.clashCore.smartConfig.lgbmAutoUpdate")}
                </Typography>
                <Switch
                  size="small"
                  checked={lgbm_auto_update ?? false}
                  onChange={(_, c) => patchVerge({ lgbm_auto_update: c })}
                />
              </Box>
              <TextField
                label={t(
                  "settings.modals.clashCore.smartConfig.lgbmUpdateInterval",
                )}
                type="number"
                size="small"
                fullWidth
                defaultValue={lgbm_update_interval ?? 24}
                onBlur={(e) => {
                  const val = parseInt(e.target.value);
                  if (val > 0) patchVerge({ lgbm_update_interval: val });
                }}
              />
              <TextField
                label={t("settings.modals.clashCore.smartConfig.lgbmUrl")}
                size="small"
                fullWidth
                defaultValue={lgbm_url ?? ""}
                onBlur={(e) => patchVerge({ lgbm_url: e.target.value })}
              />
              <TextField
                label={t(
                  "settings.modals.clashCore.smartConfig.smartCollectorSize",
                )}
                type="number"
                size="small"
                fullWidth
                defaultValue={smart_collector_size ?? 50}
                onBlur={(e) => {
                  const val = parseInt(e.target.value);
                  if (val > 0) patchVerge({ smart_collector_size: val });
                }}
              />
            </Box>
          </Box>
        </>
      )}
    </BaseDialog>
  );
}
