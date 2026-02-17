import { BlockRounded, CloseRounded } from "@mui/icons-material";
import {
  styled,
  ListItem,
  IconButton,
  ListItemText,
  Box,
  alpha,
} from "@mui/material";
import { useLockFn } from "ahooks";
import dayjs from "dayjs";
import { useTranslation } from "react-i18next";
import { closeConnection, smartBlockConnection } from "tauri-plugin-mihomo-api";

import { useVerge } from "@/hooks/use-verge";
import parseTraffic from "@/utils/parse-traffic";

const Tag = styled("span")(({ theme }) => ({
  fontSize: "10px",
  padding: "0 4px",
  lineHeight: 1.375,
  border: "1px solid",
  borderRadius: 4,
  borderColor: alpha(theme.palette.text.secondary, 0.35),
  marginTop: "4px",
  marginRight: "4px",
}));

interface Props {
  value: IConnectionsItem;
  closed: boolean;
  onShowDetail?: () => void;
}

export const ConnectionItem = (props: Props) => {
  const { value, closed, onShowDetail } = props;

  const { id, metadata, chains, start, curUpload, curDownload } = value;
  const { t } = useTranslation();

  const { verge } = useVerge();
  const isSmartCore = verge?.clash_core === "verge-mihomo-smart";

  const onDelete = useLockFn(async () => closeConnection(id));
  const onSmartBlock = useLockFn(async () => smartBlockConnection(id));
  const showTraffic = curUpload! >= 100 || curDownload! >= 100;

  return (
    <ListItem
      dense
      sx={{ borderBottom: "1px solid var(--divider-color)" }}
      secondaryAction={
        !closed && (
          <Box sx={{ display: "flex" }}>
            {isSmartCore && (
              <IconButton
                color="warning"
                onClick={onSmartBlock}
                title={t("connections.components.actions.smartBlock")}
                aria-label={t("connections.components.actions.smartBlock")}
              >
                <BlockRounded />
              </IconButton>
            )}
            <IconButton
              edge="end"
              color="inherit"
              onClick={onDelete}
              title={t("connections.components.actions.closeConnection")}
              aria-label={t("connections.components.actions.closeConnection")}
            >
              <CloseRounded />
            </IconButton>
          </Box>
        )
      }
    >
      <ListItemText
        sx={{ userSelect: "text", cursor: "pointer" }}
        primary={metadata.host || metadata.destinationIP}
        onClick={onShowDetail}
        secondary={
          <Box sx={{ display: "flex", flexWrap: "wrap" }}>
            <Tag sx={{ textTransform: "uppercase", color: "success" }}>
              {metadata.network}
            </Tag>

            <Tag>{metadata.type}</Tag>

            {!!metadata.process && <Tag>{metadata.process}</Tag>}

            {chains?.length > 0 && (
              <Tag>{[...chains].reverse().join(" / ")}</Tag>
            )}

            <Tag>{dayjs(start).fromNow()}</Tag>

            {showTraffic && (
              <Tag>
                {parseTraffic(curUpload!)} / {parseTraffic(curDownload!)}
              </Tag>
            )}
          </Box>
        }
      />
    </ListItem>
  );
};
