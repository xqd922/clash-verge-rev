from __future__ import annotations

from pathlib import Path

import lightgbm as lgb
import pandas as pd
from sklearn.model_selection import train_test_split
from sklearn.preprocessing import RobustScaler, StandardScaler

from go_parser import EXPECTED_FEATURE_COUNT, GoTransformParser


BASE_DIR = Path(__file__).resolve().parent
DATA_FILE = BASE_DIR / "smart_weight_data.csv"
GO_FILE = BASE_DIR / "transform.go"
MODEL_FILE = BASE_DIR / "Model.bin"

STD_SCALER_FEATURES = [
    "connect_time",
    "latency",
    "upload_mb",
    "history_upload_mb",
    "maxuploadrate_kb",
    "history_maxuploadrate_kb",
    "download_mb",
    "history_download_mb",
    "maxdownloadrate_kb",
    "history_maxdownloadrate_kb",
    "duration_minutes",
    "history_duration_minutes",
    "traffic_ratio",
    "traffic_density",
]
ROBUST_SCALER_FEATURES = ["success", "failure"]

LGBM_PARAMS = {
    "objective": "regression",
    "metric": "rmse",
    "n_estimators": 1000,
    "learning_rate": 0.03,
    "random_state": 42,
    "n_jobs": -1,
    "device": "gpu",
}
EARLY_STOPPING_ROUNDS = 100
TRANSFORM_TAIL_LIMIT_BYTES = 16_384


def validate_feature_contract(feature_order: list[str]) -> None:
    if len(feature_order) != EXPECTED_FEATURE_COUNT:
        raise ValueError(
            f"特征数量必须为 {EXPECTED_FEATURE_COUNT}，实际为 {len(feature_order)}。"
        )
    if len(set(feature_order)) != EXPECTED_FEATURE_COUNT:
        raise ValueError("特征名称必须唯一。")

    transformed_features = STD_SCALER_FEATURES + ROBUST_SCALER_FEATURES
    duplicate_transforms = {
        feature
        for feature in transformed_features
        if transformed_features.count(feature) > 1
    }
    if duplicate_transforms:
        raise ValueError(f"缩放器特征重复：{sorted(duplicate_transforms)}")

    missing_features = [
        feature for feature in transformed_features if feature not in feature_order
    ]
    if missing_features:
        raise ValueError(f"transform.go 缺少缩放器需要的特征：{missing_features}")


def load_and_clean_data(file_path: Path) -> pd.DataFrame | None:
    print(f"--> 正在加载数据: {file_path}")
    try:
        data = pd.read_csv(file_path)
    except FileNotFoundError:
        print(f"    错误: 数据文件 '{file_path}' 未找到!")
        return None
    except (OSError, pd.errors.EmptyDataError) as error:
        print(f"    错误: 无法读取数据文件: {error}")
        return None

    print(f"    原始数据加载成功，共 {len(data)} 条。")
    if "weight" not in data.columns:
        print("    错误: 数据文件缺少目标列 'weight'。")
        return None

    data = data.dropna(subset=["weight"]).copy()
    data["weight"] = pd.to_numeric(data["weight"], errors="coerce")
    data = data.dropna(subset=["weight"])
    data = data[data["weight"] > 0].copy()
    print(f"    清洗后剩余 {len(data)} 条有效记录。")

    if data.empty:
        print("    错误: 清洗后没有可用于训练的数据。")
        return None
    return data


def extract_features_from_preprocessed(
    data: pd.DataFrame, feature_order: list[str]
) -> tuple[pd.DataFrame, pd.Series] | None:
    print("--> 正在从预处理数据中提取特征 (X) 和目标 (y)...")
    missing_columns = [
        column for column in [*feature_order, "weight"] if column not in data.columns
    ]
    if missing_columns:
        print(f"    错误: 数据文件缺少必要列: {missing_columns}")
        return None

    try:
        features = data.loc[:, feature_order].apply(pd.to_numeric, errors="raise")
        target = pd.to_numeric(data["weight"], errors="raise")
    except (TypeError, ValueError) as error:
        print(f"    错误: 特征或目标包含非数值数据: {error}")
        return None

    print(f"    成功按 transform.go 顺序提取 {features.shape[1]} 个特征。")
    return features, target


def apply_feature_transforms(
    features: pd.DataFrame,
) -> tuple[pd.DataFrame, StandardScaler, RobustScaler]:
    print("--> 正在进行特征变换...")
    scaled_features = features.copy()

    standard_scaler = StandardScaler()
    scaled_features.loc[:, STD_SCALER_FEATURES] = standard_scaler.fit_transform(
        scaled_features.loc[:, STD_SCALER_FEATURES]
    )
    print(f"    已应用 StandardScaler 到 {len(STD_SCALER_FEATURES)} 个特征。")

    robust_scaler = RobustScaler()
    scaled_features.loc[:, ROBUST_SCALER_FEATURES] = robust_scaler.fit_transform(
        scaled_features.loc[:, ROBUST_SCALER_FEATURES]
    )
    print(f"    已应用 RobustScaler 到 {len(ROBUST_SCALER_FEATURES)} 个特征。")

    return scaled_features, standard_scaler, robust_scaler


def train_model(
    train_features: pd.DataFrame,
    train_target: pd.Series,
    test_features: pd.DataFrame,
    test_target: pd.Series,
) -> lgb.Booster:
    print("--> 正在训练 LightGBM 模型...")
    train_data = lgb.Dataset(train_features, label=train_target)
    test_data = lgb.Dataset(test_features, label=test_target, reference=train_data)

    return lgb.train(
        LGBM_PARAMS,
        train_data,
        valid_sets=[test_data],
        callbacks=[lgb.early_stopping(EARLY_STOPPING_ROUNDS, verbose=True)],
    )


def serialize_float_values(values: object) -> str:
    return ",".join(format(float(value), ".17g") for value in values)


def build_transforms_block(
    standard_scaler: StandardScaler,
    robust_scaler: RobustScaler,
    feature_order: list[str],
) -> str:
    standard_indices = [feature_order.index(name) for name in STD_SCALER_FEATURES]
    robust_indices = [feature_order.index(name) for name in ROBUST_SCALER_FEATURES]

    order_block = "[order]\n" + "".join(
        f"{index}={name}\n" for index, name in enumerate(feature_order)
    ) + "[/order]\n"

    definitions_block = (
        "[definitions]\n"
        "std_type=StandardScaler\n"
        f"std_features={','.join(map(str, standard_indices))}\n"
        f"std_mean={serialize_float_values(standard_scaler.mean_)}\n"
        f"std_scale={serialize_float_values(standard_scaler.scale_)}\n\n"
        "robust_type=RobustScaler\n"
        f"robust_features={','.join(map(str, robust_indices))}\n"
        f"robust_center={serialize_float_values(robust_scaler.center_)}\n"
        f"robust_scale={serialize_float_values(robust_scaler.scale_)}\n"
        "[/definitions]\n"
    )

    transformed_indices = set(standard_indices + robust_indices)
    untransformed_features = ",".join(
        f"{index}:{name}"
        for index, name in enumerate(feature_order)
        if index not in transformed_indices
    )

    transforms_block = (
        "\n\nend of trees\n\n"
        "[transforms]\n"
        f"{order_block}"
        f"{definitions_block}"
        f"untransformed_features={untransformed_features}\n"
        "transform=true\n"
        "[/transforms]\n"
    )
    if len(transforms_block.encode("utf-8")) >= TRANSFORM_TAIL_LIMIT_BYTES:
        raise ValueError(
            "变换配置超过 Go 端末尾读取窗口，transform.go 将无法找到完整配置。"
        )
    return transforms_block


def save_model_and_config(
    model: lgb.Booster,
    standard_scaler: StandardScaler,
    robust_scaler: RobustScaler,
    feature_order: list[str],
) -> None:
    print("--> 正在保存模型及配置...")
    if model.num_feature() != EXPECTED_FEATURE_COUNT:
        raise ValueError(
            f"模型特征数量为 {model.num_feature()}，预期为 {EXPECTED_FEATURE_COUNT}。"
        )

    best_iteration = model.best_iteration or model.current_iteration()
    model.save_model(str(MODEL_FILE), num_iteration=best_iteration)
    print(f"    模型主体已保存为文本格式到: {MODEL_FILE}")

    transforms_block = build_transforms_block(
        standard_scaler, robust_scaler, feature_order
    )
    with MODEL_FILE.open("a", encoding="utf-8") as model_file:
        model_file.write(transforms_block)
    print("    30 特征变换配置已成功附加到模型文件末尾。")


def main() -> None:
    print("--- Mihomo 模型训练开始 ---")

    try:
        feature_order = GoTransformParser(GO_FILE).get_feature_order()
        validate_feature_contract(feature_order)
    except (FileNotFoundError, ValueError) as error:
        print(f"初始化失败: {error}")
        return

    full_data = load_and_clean_data(DATA_FILE)
    if full_data is None:
        return

    extracted_data = extract_features_from_preprocessed(full_data, feature_order)
    if extracted_data is None:
        return
    features, target = extracted_data

    scaled_features, standard_scaler, robust_scaler = apply_feature_transforms(
        features
    )
    train_features, test_features, train_target, test_target = train_test_split(
        scaled_features, target, test_size=0.2, random_state=42
    )
    model = train_model(
        train_features, train_target, test_features, test_target
    )
    save_model_and_config(
        model, standard_scaler, robust_scaler, feature_order
    )

    print("\n🎉 --- 训练全部完成 --- 🎉")
    print(f"最终模型 '{MODEL_FILE}' 已生成，可以部署。")


if __name__ == "__main__":
    main()
