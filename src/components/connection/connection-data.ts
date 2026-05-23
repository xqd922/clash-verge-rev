export type ConnectionOrder = "Default" | "Upload Speed" | "Download Speed";

export const CONNECTION_ORDER_LABELS: ConnectionOrder[] = [
  "Default",
  "Upload Speed",
  "Download Speed",
];

export const NUMERIC_CONNECTION_SORTING_ORDER: ("desc" | "asc" | null)[] = [
  "desc",
  "asc",
  null,
];

const compareByNewestStart = (a: IConnectionsItem, b: IConnectionsItem) =>
  new Date(b.start || "0").getTime() - new Date(a.start || "0").getTime();

const compareByUploadSpeed = (a: IConnectionsItem, b: IConnectionsItem) =>
  (b.curUpload ?? 0) - (a.curUpload ?? 0);

const compareByDownloadSpeed = (a: IConnectionsItem, b: IConnectionsItem) =>
  (b.curDownload ?? 0) - (a.curDownload ?? 0);

const orderConnections = (connections: IConnectionsItem[], order: string) => {
  switch (order) {
    case "Upload Speed":
      return connections.sort(compareByUploadSpeed);
    case "Download Speed":
      return connections.sort(compareByDownloadSpeed);
    case "Default":
    default:
      return connections.sort(compareByNewestStart);
  }
};

export const selectConnections = (
  connections: IConnectionsItem[],
  match: (value: string) => boolean,
  order: string
) => {
  const filtered = connections.filter((conn) =>
    match(conn.metadata.host || conn.metadata.destinationIP || "")
  );
  const ordered = orderConnections(filtered, order);

  let download = 0;
  let upload = 0;
  ordered.forEach((conn) => {
    download += conn.download;
    upload += conn.upload;
  });

  return { connections: ordered, download, upload };
};
