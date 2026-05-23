export interface SelectedProxyItemSx {
  boxShadow: string;
  bgcolor: string;
}

export function getSelectedProxyItemSx(
  selectColor: string,
  bgcolor: string
): SelectedProxyItemSx {
  return {
    boxShadow: `inset 3px 0 0 ${selectColor}`,
    bgcolor,
  };
}
