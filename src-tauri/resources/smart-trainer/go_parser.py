from __future__ import annotations

import re
from pathlib import Path
from typing import Final


EXPECTED_FEATURE_COUNT: Final = 30


class GoTransformParser:
    def __init__(self, go_file_path: str | Path):
        self.go_file_path = Path(go_file_path)
        try:
            self.content = self.go_file_path.read_text(encoding="utf-8")
        except FileNotFoundError as error:
            raise FileNotFoundError(
                f"错误: Go 文件 '{self.go_file_path}' 未找到！请确保它和脚本在同一目录。"
            ) from error

        print(f"成功读取 Go 文件: {self.go_file_path}")
        self.feature_order = self._parse_feature_order()

    def _parse_feature_order(self) -> list[str]:
        print("正在解析 getDefaultFeatureOrder...")
        match = re.search(
            r"func\s+getDefaultFeatureOrder\s*\(\s*\)\s*"
            r"map\s*\[\s*int\s*\]\s*string\s*\{\s*"
            r"return\s+map\s*\[\s*int\s*\]\s*string\s*\{"
            r"(?P<body>.*?)\}\s*\}",
            self.content,
            re.DOTALL,
        )
        if not match:
            raise ValueError("未能从 transform.go 解析 getDefaultFeatureOrder 函数。")

        features = re.findall(r'(\d+)\s*:\s*"([^"]+)"', match.group("body"))
        if not features:
            raise ValueError("getDefaultFeatureOrder 中没有可解析的特征。")

        feature_order_by_index: dict[int, str] = {}
        for raw_index, name in features:
            index = int(raw_index)
            if index in feature_order_by_index:
                raise ValueError(f"getDefaultFeatureOrder 中存在重复索引：{index}")
            feature_order_by_index[index] = name

        expected_indices = list(range(EXPECTED_FEATURE_COUNT))
        actual_indices = sorted(feature_order_by_index)
        if actual_indices != expected_indices:
            raise ValueError(
                "transform.go 特征索引必须连续为 "
                f"0..{EXPECTED_FEATURE_COUNT - 1}，实际为：{actual_indices}"
            )

        feature_order = [feature_order_by_index[index] for index in expected_indices]
        if len(set(feature_order)) != EXPECTED_FEATURE_COUNT:
            raise ValueError("getDefaultFeatureOrder 中存在重复特征名称。")

        print(f"成功解析出 {len(feature_order)} 个连续且唯一的特征。")
        return feature_order

    def get_feature_order(self) -> list[str]:
        return self.feature_order.copy()
