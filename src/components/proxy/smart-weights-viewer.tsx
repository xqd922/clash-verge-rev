import { CloseRounded } from "@mui/icons-material";
import {
  Box,
  Chip,
  CircularProgress,
  Dialog,
  DialogContent,
  DialogTitle,
  IconButton,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
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

const rankColor = (rank: string) => {
  switch (rank) {
    case "MostUsed":
      return "success";
    case "OccasionalUsed":
      return "warning";
    case "RarelyUsed":
      return "default";
    default:
      return "default";
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

const barColor = (rank: string) => {
  switch (rank) {
    case "MostUsed":
      return "success.main";
    case "OccasionalUsed":
      return "warning.main";
    default:
      return "text.disabled";
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

  const maxWeight =
    weights.length > 0 ? Math.max(...weights.map((w) => w.weight), 1) : 1;

  return (
    <Dialog open={open} onClose={onClose} maxWidth="sm" fullWidth>
      <DialogTitle
        sx={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <Typography variant="h6">
          {t("proxies.page.smart.weightsTitle")} - {groupName}
        </Typography>
        <IconButton size="small" onClick={onClose}>
          <CloseRounded />
        </IconButton>
      </DialogTitle>
      <DialogContent>
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
          <TableContainer sx={{ maxHeight: 400 }}>
            <Table size="small" stickyHeader>
              <TableHead>
                <TableRow>
                  <TableCell sx={{ width: 30 }}>#</TableCell>
                  <TableCell>{t("proxies.page.smart.nodeName")}</TableCell>
                  <TableCell align="center">
                    {t("proxies.page.smart.rank")}
                  </TableCell>
                  <TableCell align="right" sx={{ width: 60 }}>
                    {t("proxies.page.smart.weight")}
                  </TableCell>
                  <TableCell sx={{ width: "30%" }}></TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {weights.map((entry, index) => (
                  <TableRow
                    key={entry.name}
                    sx={
                      index === 0
                        ? {
                            bgcolor: "action.selected",
                          }
                        : undefined
                    }
                  >
                    <TableCell>
                      <Typography
                        variant="body2"
                        fontWeight={index === 0 ? "bold" : "normal"}
                        color={index === 0 ? "primary" : "text.secondary"}
                      >
                        {index + 1}
                      </Typography>
                    </TableCell>
                    <TableCell>
                      <Typography
                        variant="body2"
                        fontWeight={index === 0 ? "bold" : "normal"}
                      >
                        {entry.name}
                      </Typography>
                    </TableCell>
                    <TableCell align="center">
                      {entry.rank && (
                        <Chip
                          label={rankLabel(entry.rank, t)}
                          color={rankColor(entry.rank) as any}
                          size="small"
                          variant="outlined"
                          sx={{ fontSize: "0.7rem", height: 20 }}
                        />
                      )}
                    </TableCell>
                    <TableCell align="right">
                      <Typography
                        variant="body2"
                        fontWeight={index === 0 ? "bold" : "normal"}
                      >
                        {entry.weight}
                      </Typography>
                    </TableCell>
                    <TableCell>
                      <Box
                        sx={{
                          height: 8,
                          borderRadius: 4,
                          bgcolor: barColor(entry.rank),
                          width: `${(entry.weight / maxWeight) * 100}%`,
                          minWidth: 4,
                          opacity: 0.8,
                        }}
                      />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        )}
      </DialogContent>
    </Dialog>
  );
};
