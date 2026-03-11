import { CloseRounded } from "@mui/icons-material";
import {
  alpha,
  Box,
  Chip,
  CircularProgress,
  Dialog,
  DialogContent,
  DialogTitle,
  IconButton,
  LinearProgress,
  Typography,
} from "@mui/material";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import useSWR from "swr";
import { getSmartWeights } from "tauri-plugin-mihomo-api";

interface Props {
  groupName: string;
  open: boolean;
  onClose: () => void;
}

interface WeightEntry {
  name: string;
  weight: number;
  rank: string;
}

const rankConfig = (rank: string) => {
  switch (rank) {
    case "MostUsed":
      return { color: "success" as const, bgKey: "success.main" };
    case "OccasionalUsed":
      return { color: "warning" as const, bgKey: "warning.main" };
    default:
      return { color: "default" as const, bgKey: "text.disabled" };
  }
};

const rankLabel = (rank: string, t: (key: string) => string) => {
  switch (rank) {
    case "MostUsed":
      return t("proxies.page.smart.rankMostUsed");
    case "OccasionalUsed":
      return t("proxies.page.smart.rankOccasional");
    case "RarelyUsed":
      return t("proxies.page.smart.rankRarely");
    default:
      return rank;
  }
};

const parseWeights = (data: any): WeightEntry[] => {
  const entries: WeightEntry[] = [];
  if (data && typeof data === "object") {
    const list = Array.isArray(data.weights) ? data.weights : [];
    for (const item of list) {
      if (item && typeof item === "object" && item.Name) {
        entries.push({
          name: item.Name,
          weight: Number(item.Weight ?? 0),
          rank: String(item.Rank ?? ""),
        });
      }
    }
  }
  entries.sort((a, b) => b.weight - a.weight);
  return entries;
};

export const SmartWeightsViewer = ({ groupName, open, onClose }: Props) => {
  const { t } = useTranslation();

  const { data, error, isLoading } = useSWR(
    open && groupName ? `smartWeights:${groupName}` : null,
    () => getSmartWeights(groupName),
    { refreshInterval: 5000 },
  );

  const weights = useMemo(() => parseWeights(data), [data]);

  const totalWeight = useMemo(
    () => weights.reduce((sum, w) => sum + w.weight, 0) || 1,
    [weights],
  );

  const maxWeight = useMemo(
    () =>
      weights.length > 0 ? Math.max(...weights.map((w) => w.weight), 1) : 1,
    [weights],
  );

  const rankStats = useMemo(() => {
    const stats = { MostUsed: 0, OccasionalUsed: 0, RarelyUsed: 0 };
    for (const w of weights) {
      if (w.rank in stats) stats[w.rank as keyof typeof stats]++;
    }
    return stats;
  }, [weights]);

  return (
    <Dialog open={open} onClose={onClose} maxWidth="sm" fullWidth>
      <DialogTitle
        sx={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          pb: 1,
        }}
      >
        <Typography variant="h6" fontSize={16}>
          {t("proxies.page.smart.weightsTitle")} - {groupName}
        </Typography>
        <IconButton size="small" onClick={onClose}>
          <CloseRounded />
        </IconButton>
      </DialogTitle>
      <DialogContent sx={{ px: 2, pb: 2 }}>
        {isLoading && (
          <Box sx={{ display: "flex", justifyContent: "center", py: 4 }}>
            <CircularProgress />
          </Box>
        )}
        {error && (
          <Typography color="error" sx={{ py: 2 }}>
            {String(error)}
          </Typography>
        )}
        {!isLoading && !error && weights.length === 0 && (
          <Typography
            color="text.secondary"
            sx={{ py: 2, textAlign: "center" }}
          >
            {t("proxies.page.smart.noData")}
          </Typography>
        )}
        {!isLoading && !error && weights.length > 0 && (
          <>
            {/* Summary chips */}
            <Box sx={{ display: "flex", gap: 1, mb: 1.5, flexWrap: "wrap" }}>
              <Chip
                label={`${rankStats.MostUsed} ${t("proxies.page.smart.rankMostUsed")}`}
                color="success"
                size="small"
                variant="outlined"
              />
              <Chip
                label={`${rankStats.OccasionalUsed} ${t("proxies.page.smart.rankOccasional")}`}
                color="warning"
                size="small"
                variant="outlined"
              />
              <Chip
                label={`${rankStats.RarelyUsed} ${t("proxies.page.smart.rankRarely")}`}
                size="small"
                variant="outlined"
              />
            </Box>

            {/* Node cards */}
            <Box
              sx={{
                maxHeight: 380,
                overflowY: "auto",
                display: "flex",
                flexDirection: "column",
                gap: 0.75,
              }}
            >
              {weights.map((entry, index) => {
                const config = rankConfig(entry.rank);
                const percent = Math.round((entry.weight / totalWeight) * 100);
                const isTop = index === 0;

                return (
                  <Box
                    key={entry.name}
                    sx={[
                      {
                        px: 1.5,
                        py: 1,
                        borderRadius: 1.5,
                        border: "1px solid",
                        borderColor: "divider",
                        transition: "all 0.2s",
                      },
                      isTop && {
                        borderColor: config.bgKey,
                        bgcolor: (theme) =>
                          alpha(
                            theme.palette.success.main,
                            theme.palette.mode === "light" ? 0.06 : 0.12,
                          ),
                      },
                    ]}
                  >
                    {/* Top row: rank + name + percent */}
                    <Box
                      sx={{
                        display: "flex",
                        alignItems: "center",
                        gap: 1,
                        mb: 0.5,
                      }}
                    >
                      <Typography
                        variant="body2"
                        sx={{
                          width: 20,
                          textAlign: "center",
                          fontWeight: isTop ? "bold" : "normal",
                          color: isTop ? "primary.main" : "text.secondary",
                          fontSize: 12,
                        }}
                      >
                        {index + 1}
                      </Typography>
                      <Typography
                        variant="body2"
                        sx={{
                          flex: 1,
                          fontWeight: isTop ? 600 : 400,
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                          fontSize: 13,
                        }}
                      >
                        {entry.name}
                      </Typography>
                      <Chip
                        label={rankLabel(entry.rank, t)}
                        color={config.color}
                        size="small"
                        variant="outlined"
                        sx={{ fontSize: "0.65rem", height: 18 }}
                      />
                      <Typography
                        variant="body2"
                        sx={{
                          minWidth: 36,
                          textAlign: "right",
                          fontWeight: isTop ? 600 : 400,
                          color: isTop ? "primary.main" : "text.secondary",
                          fontSize: 12,
                        }}
                      >
                        {percent}%
                      </Typography>
                    </Box>
                    {/* Progress bar */}
                    <Box sx={{ pl: 3.5, pr: 0.5 }}>
                      <LinearProgress
                        variant="determinate"
                        value={(entry.weight / maxWeight) * 100}
                        color={
                          config.color === "default" ? "inherit" : config.color
                        }
                        sx={{
                          height: 4,
                          borderRadius: 2,
                          bgcolor: (theme) =>
                            alpha(
                              theme.palette.text.disabled,
                              theme.palette.mode === "light" ? 0.08 : 0.15,
                            ),
                          "& .MuiLinearProgress-bar": {
                            borderRadius: 2,
                            ...(config.color === "default" && {
                              bgcolor: "text.disabled",
                            }),
                          },
                        }}
                      />
                    </Box>
                  </Box>
                );
              })}
            </Box>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
};
