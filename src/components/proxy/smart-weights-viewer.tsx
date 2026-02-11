import { CloseRounded } from "@mui/icons-material";
import {
  Box,
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
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getSmartWeights } from "tauri-plugin-mihomo-api";

interface Props {
  groupName: string;
  open: boolean;
  onClose: () => void;
}

interface WeightEntry {
  name: string;
  weight: number;
}

export const SmartWeightsViewer = ({ groupName, open, onClose }: Props) => {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [weights, setWeights] = useState<WeightEntry[]>([]);

  useEffect(() => {
    if (!open || !groupName) return;

    let cancelled = false;
    const fetchWeights = async () => {
      setLoading(true);
      setError(null);
      try {
        const data = await getSmartWeights(groupName);
        if (cancelled) return;

        // Parse the response - expected format varies, handle common cases
        const entries: WeightEntry[] = [];
        if (data && typeof data === "object") {
          for (const [key, value] of Object.entries(data)) {
            if (typeof value === "number") {
              entries.push({ name: key, weight: value });
            } else if (typeof value === "object" && value !== null) {
              // Nested weight object (e.g., { weight: number, ... })
              const w = (value as any).weight ?? (value as any).score ?? 0;
              entries.push({ name: key, weight: Number(w) });
            }
          }
        }
        entries.sort((a, b) => b.weight - a.weight);
        setWeights(entries);
      } catch (err: any) {
        if (!cancelled) {
          setError(String(err));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    fetchWeights();
    return () => {
      cancelled = true;
    };
  }, [open, groupName]);

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
        {loading && (
          <Box sx={{ display: "flex", justifyContent: "center", py: 4 }}>
            <CircularProgress />
          </Box>
        )}
        {error && (
          <Typography color="error" sx={{ py: 2 }}>
            {error}
          </Typography>
        )}
        {!loading && !error && weights.length === 0 && (
          <Typography
            color="text.secondary"
            sx={{ py: 2, textAlign: "center" }}
          >
            {t("proxies.page.smart.noData")}
          </Typography>
        )}
        {!loading && !error && weights.length > 0 && (
          <TableContainer sx={{ maxHeight: 400 }}>
            <Table size="small" stickyHeader>
              <TableHead>
                <TableRow>
                  <TableCell>{t("proxies.page.smart.nodeName")}</TableCell>
                  <TableCell align="right">
                    {t("proxies.page.smart.weight")}
                  </TableCell>
                  <TableCell sx={{ width: "40%" }}></TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {weights.map((entry) => (
                  <TableRow key={entry.name}>
                    <TableCell>{entry.name}</TableCell>
                    <TableCell align="right">
                      {entry.weight.toFixed(2)}
                    </TableCell>
                    <TableCell>
                      <Box
                        sx={{
                          height: 8,
                          borderRadius: 4,
                          bgcolor: "primary.main",
                          width: `${(entry.weight / maxWeight) * 100}%`,
                          minWidth: 4,
                          opacity: 0.7,
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
