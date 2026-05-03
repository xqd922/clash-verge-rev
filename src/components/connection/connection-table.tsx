import dayjs from "dayjs";
import { useMemo, useState } from "react";
import { DataGrid, GridColDef } from "@mui/x-data-grid";
import { useThemeMode } from "@/services/states";
import { truncateStr } from "@/utils/truncate-str";
import parseTraffic from "@/utils/parse-traffic";
import { t } from "i18next";

interface Props {
  connections: IConnectionsItem[];
  onShowDetail: (data: IConnectionsItem) => void;
}

export const ConnectionTable = (props: Props) => {
  const { connections, onShowDetail } = props;
  const mode = useThemeMode();
  const isDark = mode === "light" ? false : true;
  // 与 connections.tsx 外层 Box / BasePage .base-container 的写死 hex 对齐，
  // 保留外层 #ffffff "白色 paper 卡片" 浮在 #f0f0f0 灰底上的视觉。
  // theme.palette.background.paper 在 light 模式是 #F5F5F5，会把外层白底盖掉。
  const backgroundColor = isDark ? "#282A36" : "#ffffff";

  const [columnVisible, setColumnVisible] = useState<
    Partial<Record<keyof IConnectionsItem, boolean>>
  >({});

  const [columns] = useState<GridColDef[]>([
    { field: "host", headerName: t("Host"), flex: 220, minWidth: 220 },
    {
      field: "download",
      headerName: t("Downloaded"),
      width: 88,
      align: "right",
      headerAlign: "right",
      sortingOrder: ["desc", "asc", null],
      valueFormatter: (value: number) => parseTraffic(value).join(" "),
    },
    {
      field: "upload",
      headerName: t("Uploaded"),
      width: 88,
      align: "right",
      headerAlign: "right",
      sortingOrder: ["desc", "asc", null],
      valueFormatter: (value: number) => parseTraffic(value).join(" "),
    },
    {
      field: "dlSpeed",
      headerName: t("DL Speed"),
      width: 88,
      align: "right",
      headerAlign: "right",
      sortingOrder: ["desc", "asc", null],
      valueFormatter: (value: number) => parseTraffic(value).join(" ") + "/s",
    },
    {
      field: "ulSpeed",
      headerName: t("UL Speed"),
      width: 88,
      align: "right",
      headerAlign: "right",
      sortingOrder: ["desc", "asc", null],
      valueFormatter: (value: number) => parseTraffic(value).join(" ") + "/s",
    },
    { field: "chains", headerName: t("Chains"), flex: 360, minWidth: 360 },
    { field: "rule", headerName: t("Rule"), flex: 300, minWidth: 250 },
    { field: "process", headerName: t("Process"), flex: 240, minWidth: 120 },
    {
      field: "time",
      headerName: t("Time"),
      flex: 120,
      minWidth: 100,
      align: "right",
      headerAlign: "right",
      sortComparator: (v1: string, v2: string) =>
        new Date(v2).getTime() - new Date(v1).getTime(),
      valueFormatter: (value: number) => dayjs(value).fromNow(),
    },
    { field: "source", headerName: t("Source"), flex: 200, minWidth: 130 },
    {
      field: "destinationIP",
      headerName: t("Destination IP"),
      flex: 200,
      minWidth: 130,
    },
    { field: "type", headerName: t("Type"), flex: 160, minWidth: 100 },
  ]);

  const connRows = useMemo(() => {
    return connections.map((each) => {
      const { metadata, rulePayload } = each;
      const chains = [...each.chains].reverse().join(" / ");
      const rule = rulePayload ? `${each.rule}(${rulePayload})` : each.rule;
      return {
        id: each.id,
        host: metadata.host
          ? `${metadata.host}:${metadata.destinationPort}`
          : `${metadata.destinationIP}:${metadata.destinationPort}`,
        download: each.download,
        upload: each.upload,
        dlSpeed: each.curDownload,
        ulSpeed: each.curUpload,
        chains,
        rule,
        process: truncateStr(metadata.process || metadata.processPath),
        time: each.start,
        source: `${metadata.sourceIP}:${metadata.sourcePort}`,
        destinationIP: metadata.destinationIP,
        type: `${metadata.type}(${metadata.network})`,
        connectionData: each,
      };
    });
  }, [connections]);

  return (
    <DataGrid
      hideFooter
      rows={connRows}
      columns={columns}
      onRowClick={(e) => onShowDetail(e.row.connectionData)}
      density="compact"
      sx={{
        border: "none",
        bgcolor: backgroundColor,
        "div:focus": { outline: "none !important" },
        "& .MuiDataGrid-main": { bgcolor: backgroundColor },
        "& .MuiDataGrid-columnHeaders": { bgcolor: backgroundColor },
        "& .MuiDataGrid-virtualScroller": { bgcolor: backgroundColor },
        "& .MuiDataGrid-overlayWrapper": { bgcolor: backgroundColor },
        "& div[aria-rowindex]": {
          backgroundColor: `${backgroundColor} !important`,
        },
      }}
      columnVisibilityModel={columnVisible}
      onColumnVisibilityModelChange={(e) => setColumnVisible(e)}
    />
  );
};
