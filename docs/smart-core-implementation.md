# Smart 核心深度优化 — 实现文档

## 概述

本文档详细记录了 Clash Verge Rev 对 mihomo Smart 核心的全面支持实现过程。Smart 核心是 [vernesong/mihomo](https://github.com/vernesong/mihomo) 的分支，引入了 `type: smart` 代理组，通过 AI 多维度分析（延迟、稳定性、速度、丢包率、场景识别）替代传统 url-test 的纯延迟排序，并集成 LightGBM 机器学习模型进行节点权重预测。

实现共涉及 **4 大功能模块**，修改 **70+ 个文件**，覆盖 Rust 后端、TypeScript 前端、Tauri 插件层和 13 种语言的国际化。

---

## 目录

1. [基础兼容性修复](#1-基础兼容性修复)
2. [Smart 专用 API 支持](#2-smart-专用-api-支持)
3. [一键转换 Smart 组](#3-一键转换-smart-组)
4. [Smart 配置项 UI](#4-smart-配置项-ui)
5. [权限注册](#5-权限注册)
6. [国际化](#6-国际化)

---

## 1. 基础兼容性修复

### 问题

Rust 后端的 `models.rs` 已有 `Smart` 枚举值，但前端多处硬编码了代理组类型列表，未包含 `Smart`，导致 Smart 组在 UI 中无法正确显示和交互。

### 1.1 TypeScript 类型绑定

**文件**: `crates/tauri-plugin-mihomo/guest-js/bindings/ProxyType.ts`

Rust 通过 `ts-rs` 自动生成 TypeScript 类型绑定，但生成的联合类型缺少 `Smart`。手动追加：

```typescript
// 修改前
export type ProxyType = ... | "LoadBalance";

// 修改后
export type ProxyType = ... | "LoadBalance" | "Smart";
```

**为什么需要**：前端所有引用 `ProxyType` 的地方都依赖这个联合类型做类型检查，缺少 `Smart` 会导致类型不匹配。

### 1.2 代理选择持久化

**文件**: `src/hooks/use-profiles.ts` (line 114-120)

```typescript
const selectableTypes = new Set([
  "Selector",
  "URLTest",
  "Fallback",
  "LoadBalance",
  "Smart", // 新增
]);
```

**作用**：`selectableTypes` 控制哪些代理组类型支持"记住用户手动选择的节点"。Smart 组和 Selector 一样支持手动选节点，所以需要加入此集合。当用户在 Smart 组中手动选择了某个节点，切换配置后会自动恢复选择。

### 1.3 链式代理模式

**文件**: `src/components/proxy/proxy-groups.tsx` (line 89)

```typescript
// 修改前
? groups.filter((g) => g.type === "Selector")

// 修改后
? groups.filter((g) => g.type === "Selector" || g.type === "Smart")
```

**作用**：链式代理模式（Chain Proxy）下，只展示可手动选择节点的组。Smart 组支持手动选择，因此需要在过滤条件中包含。

### 1.4 延迟测试

**文件**: `src/components/proxy/proxy-groups.tsx` (line 288)

```typescript
// 修改前
if (!["Selector", "URLTest", "Fallback"].includes(group.type)) return;

// 修改后
if (!["Selector", "URLTest", "Fallback", "Smart"].includes(group.type)) return;
```

**作用**：控制哪些组类型支持批量延迟测试。Smart 组的节点同样需要延迟检测来辅助 AI 决策。

### 1.5 固定节点提示

**文件**: `src/components/proxy/proxy-item-mini.tsx` (line 284)

```typescript
group.type === "URLTest" || group.type === "Smart"
  ? t("proxies.page.labels.delayCheckReset")
  : "";
```

**作用**：当用户在 url-test 或 Smart 组中手动固定了某个节点时，显示📌图标。点击后取消固定，恢复自动选择。Smart 组和 url-test 一样支持此行为。

### 1.6 类型定义扩展

**文件**: `src/types/global.d.ts` (line 343-372)

```typescript
interface IProxyGroupConfig {
  type: "select" | "url-test" | "fallback" | "load-balance" | "relay" | "smart";
  // ... 原有字段 ...

  // Smart 专属字段
  "policy-priority"?: string; // 节点优先级权重，如 "Premium:0.9;SG:1.3"
  uselightgbm?: boolean; // 启用 LightGBM 机器学习模型
  collectdata?: boolean; // 收集训练数据
  "sample-rate"?: number; // 数据采样率 (0-1)
  "prefer-asn"?: boolean; // 优先选同 ASN 节点
  "lgbm-auto-update"?: boolean; // 自动更新 LightGBM 模型
  "lgbm-update-interval"?: number; // 模型更新间隔（小时）
  "lgbm-model-url"?: string; // 模型下载地址
}
```

### 1.7 Groups Editor 类型选项

**文件**: `src/components/profile/groups-editor-viewer.tsx`

两处修改：

1. **type 下拉选项** (line ~500)：在 Select 组件的 options 中追加 `"smart"`
2. **exclude-type 自动补全** (line ~841)：在 Autocomplete 的选项中追加 `"Smart"`

```typescript
// type 下拉
options={["select", "url-test", "fallback", "load-balance", "relay", "smart"]}

// exclude-type
options={["Shadowsocks", "ShadowsocksR", ..., "Smart"]}
```

### 1.8 策略标签映射

**文件**: `src/components/profile/groups-editor-viewer.tsx` (line 81)

```typescript
const PROXY_STRATEGY_LABEL_KEYS: Record<string, TranslationKey> = {
  select: "proxies.components.enums.strategies.select",
  "url-test": "proxies.components.enums.strategies.urlTest",
  fallback: "proxies.components.enums.strategies.fallback",
  "load-balance": "proxies.components.enums.strategies.loadBalance",
  relay: "proxies.components.enums.strategies.relay",
  smart: "proxies.components.enums.strategies.smart", // 新增
};
```

**作用**：在 Groups Editor 列表中，每个组类型旁显示一个描述性标签（如 "AI 智能节点选择"），方便用户理解各类型的功能区别。

---

## 2. Smart 专用 API 支持

### 背景

Smart 核心提供两个独有的 REST API 端点：

| 端点                    | 方法 | 功能                                  |
| ----------------------- | ---- | ------------------------------------- |
| `/group/{name}/weights` | GET  | 获取 Smart 组的节点权重数据           |
| `/cache/smart/flush`    | POST | 清除 Smart 缓存（权重、训练数据缓存） |

需要从 Rust Tauri 插件层 → TypeScript API → 前端 UI 完整打通。

### 2.1 Rust HTTP 方法

**文件**: `crates/tauri-plugin-mihomo/src/mihomo.rs` (line 431-458)

在 `Mihomo` impl 中，紧跟 `flush_dns()` 方法之后添加两个方法：

```rust
pub async fn get_smart_weights(&self, group_name: &str) -> Result<serde_json::Value> {
    let group_name_encode = urlencoding::encode(group_name);
    let client = self.build_request(
        Method::GET,
        &format!("/group/{group_name_encode}/weights"),
    )?;
    let response = self.send_by_protocol(client).await?;
    if !response.status().is_success() {
        let err_msg = response.json::<ErrorResponse>().await.map_or_else(
            |e| format!("get smart weights for group[{}] failed, {}", group_name, e),
            |err_res| err_res.message,
        );
        ret_failed_resp!("{}", err_msg);
    }
    Ok(response.json::<serde_json::Value>().await?)
}

pub async fn flush_smart_cache(&self) -> Result<()> {
    let client = self.build_request(Method::POST, "/cache/smart/flush")?;
    let response = self.send_by_protocol(client).await?;
    if !response.status().is_success() {
        let err_msg = response.json::<ErrorResponse>().await.map_or_else(
            |e| format!("flush smart cache failed, {}", e),
            |err_res| err_res.message,
        );
        ret_failed_resp!("{}", err_msg);
    }
    Ok(())
}
```

**设计决策**：

- `get_smart_weights` 返回 `serde_json::Value` 而非强类型结构体，因为 Smart 核心的权重数据格式可能随版本变化，使用灵活的 JSON 值更具前向兼容性。
- `group_name` 需要 URL 编码，因为组名可能包含中文或特殊字符。
- 复用现有的 `build_request` / `send_by_protocol` 模式，自动处理 HTTP/HTTPS 协议切换和认证。

### 2.2 Tauri 命令

**文件**: `crates/tauri-plugin-mihomo/src/commands.rs` (line 41-53)

```rust
#[command]
pub(crate) async fn get_smart_weights(
    state: State<'_, RwLock<Mihomo>>,
    group_name: String,
) -> Result<serde_json::Value> {
    state.read().await.get_smart_weights(&group_name).await
}

#[command]
pub(crate) async fn flush_smart_cache(
    state: State<'_, RwLock<Mihomo>>,
) -> Result<()> {
    state.read().await.flush_smart_cache().await
}
```

### 2.3 命令注册

**文件**: `crates/tauri-plugin-mihomo/src/lib.rs` (line 105-107)

在 `tauri::generate_handler![]` 宏中注册：

```rust
// smart
commands::get_smart_weights,
commands::flush_smart_cache,
```

### 2.4 TypeScript API 封装

**文件**: `crates/tauri-plugin-mihomo/guest-js/index.ts` (line 62-82)

```typescript
export async function getSmartWeights(
  groupName: string,
): Promise<Record<string, any>> {
  return await invoke<Record<string, any>>("plugin:mihomo|get_smart_weights", {
    groupName,
  });
}

export async function flushSmartCache(): Promise<void> {
  await invoke<void>("plugin:mihomo|flush_smart_cache");
}
```

**注意**：修改 `guest-js` 后需要运行 `pnpm build`（在 `crates/tauri-plugin-mihomo/` 目录下）重新编译 rollup 生成 `dist-js/`。同时需要确保 `package.json` 中的依赖指向本地路径：

```json
"tauri-plugin-mihomo-api": "file:crates/tauri-plugin-mihomo"
```

### 2.5 Smart 权重查看器

**文件**: `src/components/proxy/smart-weights-viewer.tsx`（新建）

完整的 Dialog 组件，功能包括：

1. 当弹窗打开时，调用 `getSmartWeights(groupName)` 获取权重数据
2. 解析 JSON 响应，提取节点名和权重值
3. 按权重降序排列
4. 用 MUI Table 展示，每行包含节点名、权重数值和相对比例的柱状条
5. 处理 loading、error、空数据三种状态

```typescript
interface Props {
  groupName: string;
  open: boolean;
  onClose: () => void;
}

export const SmartWeightsViewer = ({ groupName, open, onClose }: Props) => {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [weights, setWeights] = useState<WeightEntry[]>([]);

  useEffect(() => {
    if (!open || !groupName) return;
    let cancelled = false;
    const fetchWeights = async () => {
      setLoading(true);
      try {
        const data = await getSmartWeights(groupName);
        if (cancelled) return;
        const entries = Object.entries(data)
          .map(([name, weight]) => ({ name, weight: Number(weight) || 0 }))
          .sort((a, b) => b.weight - a.weight);
        setWeights(entries);
      } catch (err: any) {
        if (!cancelled) setError(String(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    fetchWeights();
    return () => {
      cancelled = true;
    };
  }, [open, groupName]);
  // ... Dialog with Table rendering
};
```

**柱状图可视化**：每个节点的权重用相对宽度的彩色条表示，最大权重为 100% 宽度，方便直观对比。

### 2.6 权重查看入口

**文件**: `src/components/proxy/proxy-head.tsx` (line 75-188)

在代理组标题栏添加查看权重的 IconButton：

```typescript
const isSmartCore = verge?.clash_core === "verge-mihomo-smart";
const isSmartGroup = groupType === "Smart";

// 仅当使用 Smart 核心且组类型为 Smart 时显示
{isSmartCore && isSmartGroup && (
  <IconButton
    size="small"
    title={t("proxies.page.tooltips.viewWeights")}
    onClick={() => setWeightsOpen(true)}
  >
    <InsightsRounded />
  </IconButton>
)}
```

**文件**: `src/components/proxy/proxy-render.tsx`

将 `group.type` 通过 `groupType` prop 传递给 `ProxyHead`：

```typescript
<ProxyHead groupType={group.type} ... />
```

### 2.7 清除 Smart 缓存按钮

**文件**: `src/components/setting/mods/clash-core-viewer.tsx` (line 125-158)

在核心管理弹窗中，当使用 Smart 核心时显示"清除缓存"按钮：

```typescript
{clash_core === "verge-mihomo-smart" && (
  <LoadingButton
    variant="contained"
    size="small"
    startIcon={<CachedRounded />}
    loading={flushingCache}
    onClick={onFlushSmartCache}
  >
    {t("settings.modals.clashCore.actions.flushSmartCache")}
  </LoadingButton>
)}
```

---

## 3. 一键转换 Smart 组

### 需求

用户的订阅配置通常使用 `url-test`、`fallback`、`load-balance` 类型。切换到 Smart 核心后，需要手动逐个修改组类型，非常不便。需要一个开关，自动将这些组转换为 `smart` 类型。

### 3.1 内置脚本

**文件**: `src-tauri/src/enhance/builtin/smart_convert.js`（新建）

```javascript
// eslint-disable-next-line unused-imports/no-unused-vars
function main(config, _name) {
  if (Array.isArray(config["proxy-groups"])) {
    config["proxy-groups"].forEach(function (group, i) {
      var type = (group.type || "").toLowerCase();
      if (
        type === "url-test" ||
        type === "fallback" ||
        type === "load-balance"
      ) {
        config["proxy-groups"][i].type = "smart";
        // 启用数据收集
        if (config["proxy-groups"][i].collectdata === undefined) {
          config["proxy-groups"][i].collectdata = true;
        }
        // 启用 LightGBM 模型
        if (config["proxy-groups"][i].uselightgbm === undefined) {
          config["proxy-groups"][i].uselightgbm = true;
        }
        // 启用模型自动更新
        if (config["proxy-groups"][i]["lgbm-auto-update"] === undefined) {
          config["proxy-groups"][i]["lgbm-auto-update"] = true;
        }
      }
    });
  }
  return config;
}
```

**关键设计决策**：

1. **ES5 语法**：Tauri 后端使用 Boa 引擎执行 JavaScript，仅支持 ES5。因此使用 `function`、`var`、`forEach` 而非箭头函数、`let`、`for...of`。
2. **条件赋值**：只在字段为 `undefined` 时赋默认值，不覆盖用户的显式配置。
3. **完整 ML 配置**：转换时自动启用 `collectdata`（数据收集）、`uselightgbm`（ML 模型）和 `lgbm-auto-update`（模型自动更新），让用户零配置即可使用完整的 Smart 能力。
4. **ESLint 忽略注释**：`main` 函数是给 Boa 引擎调用的，ESLint 会误报"定义未使用"，因此加了 `// eslint-disable-next-line`。

### 3.2 脚本注册到增强链

**文件**: `src-tauri/src/enhance/chain.rs` (line 133-149)

```rust
let smart_convert = Self::to_script(
    "verge_smart_convert",
    include_str!("./builtin/smart_convert.js"),
);

vec![
    (ChainSupport::Stable, hy_alpn),
    (ChainSupport::Stable, meta_guard),
    (ChainSupport::Alpha, hy_alpn_alpha),
    (ChainSupport::Alpha, meta_guard_alpha),
    (ChainSupport::Smart, hy_alpn_smart),
    (ChainSupport::Smart, meta_guard_smart),
    (ChainSupport::Smart, smart_convert),  // 新增
]
```

**执行时机**：`ChainSupport::Smart` 表示仅在 Smart 核心激活时运行此脚本。增强管线的执行顺序为：

```
全局 merge → 全局 script → 各订阅项 → 内置脚本 → 清理 → TUN/DNS
```

`smart_convert` 在内置脚本阶段运行，此时订阅的 proxy-groups 已经合并完毕，转换操作作用于最终配置。

### 3.3 配置开关

**文件**: `src-tauri/src/config/verge.rs`

```rust
// 字段定义 (line 164)
pub enable_smart_convert: Option<bool>,

// 默认值 (line 434)
enable_smart_convert: Some(false),

// patch 宏 (line 536)
patch!(enable_smart_convert);
```

### 3.4 条件过滤

**文件**: `src-tauri/src/enhance/mod.rs`

在 `ConfigValues` 结构体中新增 `enable_smart_convert` 字段：

```rust
struct ConfigValues {
    clash_config: Mapping,
    clash_core: Option<String>,
    enable_tun: bool,
    enable_builtin: bool,
    enable_smart_convert: bool,  // 新增
    // ...
}
```

在 `apply_builtin_scripts()` 中，当 `enable_smart_convert` 为 `false` 时跳过 `verge_smart_convert` 脚本：

```rust
fn apply_builtin_scripts(
    config: Mapping,
    builtin_items: Vec<ChainItem>,
    enable_smart_convert: bool,
) -> Mapping {
    let mut config = config;
    for item in builtin_items {
        // 当用户未开启自动转换时，跳过 smart_convert 脚本
        if item.uid == "verge_smart_convert" && !enable_smart_convert {
            continue;
        }
        // ... 正常执行脚本
    }
    config
}
```

### 3.5 开关即时生效

**文件**: `src-tauri/src/feat/config.rs` (line 120-139)

关键问题：用户切换 `enable_smart_convert` 开关后，需要立即重新生成配置并重启核心，否则开关不会生效。

在 `determine_update_flags()` 中，将 `enable_smart_convert` 和 `enable_builtin_enhanced` 加入 `restart_core_needed` 条件：

```rust
let enable_builtin_enhanced = patch.enable_builtin_enhanced;
let enable_smart_convert = patch.enable_smart_convert;

let restart_core_needed = socks_enabled.is_some()
    || http_enabled.is_some()
    || socks_port.is_some()
    || http_port.is_some()
    || mixed_port.is_some()
    || enable_external_controller.is_some()
    || enable_builtin_enhanced.is_some()   // 新增
    || enable_smart_convert.is_some();     // 新增
```

当 `RESTART_CORE` 标志被设置时，会触发 `Config::generate()` → `CoreManager::restart_core()`，完成配置重新增强和核心重启。

### 3.6 前端开关

**文件**: `src/components/setting/setting-clash.tsx` (line 270-292)

开关放在 Clash 设置页面的"Clash 内核"下方，仅在 Smart 核心时可见：

```typescript
{verge?.clash_core === "verge-mihomo-smart" && (
  <SettingItem
    label={t("settings.sections.clash.form.fields.enableSmartConvert")}
    extra={
      <TooltipIcon
        title={t("settings.sections.clash.form.tooltips.enableSmartConvert")}
      />
    }
  >
    <GuardState
      value={verge?.enable_smart_convert ?? false}
      valueProps="checked"
      onCatch={onError}
      onFormat={onSwitchFormat}
      onGuard={(e) => patchVerge({ enable_smart_convert: e })}
    >
      <Switch edge="end" />
    </GuardState>
  </SettingItem>
)}
```

**使用 `GuardState` 的原因**：`GuardState` 是项目中设置页面的标准模式，它在 `onGuard` 回调失败时自动回滚 UI 状态，提供乐观更新体验。

### 3.7 前端类型

**文件**: `src/types/global.d.ts`

```typescript
interface IVergeConfig {
  // ...
  enable_smart_convert?: boolean;
}
```

---

## 4. Smart 配置项 UI

### 需求

Smart 组支持多个独有的高级配置字段，需要在 Groups Editor 中提供 UI 控件。

### 4.1 表单默认值

**文件**: `src/components/profile/groups-editor-viewer.tsx` (line 170-178)

```typescript
const { control, watch, ...formIns } = useForm<IProxyGroupConfig>({
  defaultValues: {
    type: "select",
    name: "",
    interval: 300,
    timeout: 5000,
    "max-failed-times": 5,
    lazy: true,
    // Smart defaults
    uselightgbm: false,
    collectdata: false,
    "sample-rate": 1,
    "prefer-asn": false,
    "lgbm-auto-update": false,
    "lgbm-update-interval": 72,
    "lgbm-model-url": "",
  },
});
```

### 4.2 条件渲染

使用 `watch("type")` 监听当前选中的组类型，当 `type === "smart"` 时显示 Smart 专属字段区域：

```typescript
const currentType = watch("type");

// 在 JSX 中
{currentType === "smart" && (
  <>
    {/* policy-priority */}
    <Controller name="policy-priority" control={control}
      render={({ field }) => (
        <Item>
          <ListItemText primary={t("profiles.modals.groupsEditor.fields.policyPriority")} />
          <TextField
            placeholder="Premium:0.9;SG:1.3"
            helperText={t("profiles.modals.groupsEditor.fields.policyPriorityHint")}
            {...field}
          />
        </Item>
      )}
    />

    {/* uselightgbm - Switch */}
    <Controller name="uselightgbm" ... />

    {/* collectdata - Switch */}
    <Controller name="collectdata" ... />

    {/* sample-rate - 数字输入 (0-1, step 0.1) */}
    <Controller name="sample-rate" ... />

    {/* prefer-asn - Switch */}
    <Controller name="prefer-asn" ... />

    {/* lgbm-auto-update - Switch */}
    <Controller name="lgbm-auto-update" ... />

    {/* lgbm-update-interval - 数字输入 */}
    <Controller name="lgbm-update-interval" ... />

    {/* lgbm-model-url - 文本输入 */}
    <Controller name="lgbm-model-url" ... />
  </>
)}
```

### 4.3 各字段说明

| 字段                   | 控件类型     | 默认值 | 说明                                                               |
| ---------------------- | ------------ | ------ | ------------------------------------------------------------------ |
| `policy-priority`      | TextField    | 空     | 正则模式设置节点优先权重，如 `Premium:0.9;SG:1.3`（<1 优先级更高） |
| `uselightgbm`          | Switch       | false  | 启用 LightGBM 机器学习模型预测权重                                 |
| `collectdata`          | Switch       | false  | 收集节点性能数据用于 ML 训练，保存到 `smart_weight_data.csv`       |
| `sample-rate`          | Number (0-1) | 1      | 数据采样率，1 表示全量采集                                         |
| `prefer-asn`           | Switch       | false  | 优先选择同 ASN 的节点                                              |
| `lgbm-auto-update`     | Switch       | false  | 自动从远程下载更新 LightGBM 模型                                   |
| `lgbm-update-interval` | Number       | 72     | 模型自动更新间隔（小时）                                           |
| `lgbm-model-url`       | TextField    | 空     | 自定义 Model.bin 下载地址                                          |

---

## 5. 权限注册

### 问题

Tauri 2 采用基于权限的安全模型。每个插件命令都需要在 `permissions/` 目录下注册权限文件，并在 `default.toml` 中启用，否则前端调用时会报 `"Command not found"` 错误。

### 5.1 权限文件

**文件**: `crates/tauri-plugin-mihomo/permissions/autogenerated/commands/get_smart_weights.toml`（新建）

```toml
"$schema" = "../../schemas/schema.json"

[[permission]]
identifier = "allow-get-smart-weights"
description = "Enables the get_smart_weights command without any pre-configured scope."
commands.allow = ["get_smart_weights"]

[[permission]]
identifier = "deny-get-smart-weights"
description = "Denies the get_smart_weights command without any pre-configured scope."
commands.deny = ["get_smart_weights"]
```

**文件**: `crates/tauri-plugin-mihomo/permissions/autogenerated/commands/flush_smart_cache.toml`（新建）

同样的结构，注册 `flush_smart_cache` 命令。

### 5.2 默认权限

**文件**: `crates/tauri-plugin-mihomo/permissions/default.toml`

```toml
[default]
permissions = [
  # ... 现有权限 ...
  "allow-flush-dns",
  "allow-get-smart-weights",    # 新增
  "allow-flush-smart-cache",    # 新增
  # ...
]
```

---

## 6. 国际化

### 涉及范围

所有 13 种语言（ar, de, en, es, fa, id, jp, ko, ru, tr, tt, zh, zhtw），三类翻译文件：

| 文件            | 新增 key 数 | 内容                                                                                                                                   |
| --------------- | ----------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `proxies.json`  | 6           | smart 策略标签、权重查看器标题/空数据/节点名/权重、查看权重 tooltip                                                                    |
| `settings.json` | 4           | 自动转换字段名/tooltip、清除缓存按钮、缓存已清除通知                                                                                   |
| `profiles.json` | 10          | policy-priority、useLightGBM、collectData、sampleRate、preferAsn、lgbmAutoUpdate、lgbmUpdateInterval、lgbmModelUrl、policyPriorityHint |

### 工作流

1. 先在 `en/` 和 `zh/` 中手写翻译
2. 对其他 11 个语言使用英文作为 fallback
3. 运行 `pnpm i18n:format` 自动对齐文件结构
4. 运行 `node scripts/generate-i18n-keys.mjs` 重新生成 `TranslationKey` 联合类型
5. 运行 `npx prettier --write` 格式化生成的文件
6. 运行 `pnpm i18n:check` 确认 0 missing、0 unused

### 自动生成的类型文件

- `src/types/generated/i18n-keys.ts` — 所有翻译 key 的 const 数组 + `TranslationKey` 联合类型
- `src/types/generated/i18n-resources.ts` — 资源结构类型

这两个文件由 `scripts/generate-i18n-keys.mjs` 从 `en/` 目录的 JSON 文件自动生成。**每次修改 i18n JSON 后都必须重新生成**，否则 TypeScript 会报类型错误。

---

## 数据流总览

```
┌─────────────────────────────────────────────────────────┐
│  用户操作                                                │
│                                                         │
│  开启"自动转换为 Smart" ──→ patchVerge()                  │
│       │                         │                       │
│       │                  determine_update_flags()        │
│       │                         │                       │
│       │                  RESTART_CORE flag               │
│       │                         │                       │
│       │                  Config::generate()              │
│       │                         │                       │
│       │                  enhance_profiles()              │
│       │                         │                       │
│       │              ┌──────────┴──────────┐            │
│       │              │  增强管线            │            │
│       │              │                     │            │
│       │              │  1. 全局 merge       │            │
│       │              │  2. 全局 script      │            │
│       │              │  3. 订阅项增强       │            │
│       │              │  4. 内置脚本         │            │
│       │              │     ├─ meta_guard    │            │
│       │              │     └─ smart_convert │◄── 这里转换 │
│       │              │  5. 清理无效引用     │            │
│       │              │  6. TUN/DNS 设置     │            │
│       │              └──────────┬──────────┘            │
│       │                         │                       │
│       │                  CoreManager::restart_core()     │
│       │                         │                       │
│       │                  mihomo 重新加载配置              │
│       │                         │                       │
│       │                  前端收到 RefreshClash 事件       │
│       │                         │                       │
│       └─── 代理页显示 Smart 组 ◄─┘                       │
│                                                         │
│  点击查看权重 ──→ getSmartWeights(groupName)              │
│       │              │                                  │
│       │         Tauri invoke → Rust command              │
│       │              │                                  │
│       │         GET /group/{name}/weights                │
│       │              │                                  │
│       └─── Dialog 展示权重表格 ◄─┘                       │
│                                                         │
│  点击清除缓存 ──→ flushSmartCache()                      │
│       │              │                                  │
│       │         POST /cache/smart/flush                  │
│       │              │                                  │
│       └─── 显示成功通知 ◄──┘                             │
└─────────────────────────────────────────────────────────┘
```

---

## 文件变更清单

### Rust (7 文件 + 1 新建)

| 文件                                                     | 改动                                                       |
| -------------------------------------------------------- | ---------------------------------------------------------- |
| `crates/tauri-plugin-mihomo/src/mihomo.rs`               | +2 API 方法 (get_smart_weights, flush_smart_cache)         |
| `crates/tauri-plugin-mihomo/src/commands.rs`             | +2 Tauri 命令                                              |
| `crates/tauri-plugin-mihomo/src/lib.rs`                  | 注册 2 命令                                                |
| `src-tauri/src/enhance/chain.rs`                         | 注册 smart_convert 内置脚本                                |
| `src-tauri/src/enhance/mod.rs`                           | +enable_smart_convert 条件过滤                             |
| `src-tauri/src/config/verge.rs`                          | +enable_smart_convert 配置字段                             |
| `src-tauri/src/feat/config.rs`                           | +enable_smart_convert/enable_builtin_enhanced 触发核心重启 |
| **NEW** `src-tauri/src/enhance/builtin/smart_convert.js` | 自动转换脚本                                               |

### Frontend (10 文件 + 1 新建)

| 文件                                                        | 改动                     |
| ----------------------------------------------------------- | ------------------------ |
| `crates/tauri-plugin-mihomo/guest-js/bindings/ProxyType.ts` | +Smart 类型              |
| `crates/tauri-plugin-mihomo/guest-js/index.ts`              | +2 API 函数              |
| `src/hooks/use-profiles.ts`                                 | selectableTypes +Smart   |
| `src/components/proxy/proxy-groups.tsx`                     | 链模式+延迟测试 +Smart   |
| `src/components/proxy/proxy-head.tsx`                       | +权重查看按钮            |
| `src/components/proxy/proxy-render.tsx`                     | +groupType prop 传递     |
| `src/components/proxy/proxy-item-mini.tsx`                  | +Smart 固定节点处理      |
| `src/components/profile/groups-editor-viewer.tsx`           | +smart 类型+全部配置字段 |
| `src/components/setting/mods/clash-core-viewer.tsx`         | +清除 Smart 缓存按钮     |
| `src/components/setting/setting-clash.tsx`                  | +自动转换开关            |
| **NEW** `src/components/proxy/smart-weights-viewer.tsx`     | 权重查看器 Dialog        |

### 类型 + 权限

| 文件                                                  | 改动                                                              |
| ----------------------------------------------------- | ----------------------------------------------------------------- |
| `src/types/global.d.ts`                               | IProxyGroupConfig +Smart 字段, IVergeConfig +enable_smart_convert |
| `crates/tauri-plugin-mihomo/permissions/default.toml` | +2 权限                                                           |
| **NEW** `permissions/.../get_smart_weights.toml`      | 权限定义                                                          |
| **NEW** `permissions/.../flush_smart_cache.toml`      | 权限定义                                                          |

### i18n (39 文件)

| 文件                               | 改动                        |
| ---------------------------------- | --------------------------- |
| 13 × `src/locales/*/proxies.json`  | +smart 策略标签、权重查看器 |
| 13 × `src/locales/*/settings.json` | +自动转换、缓存清除         |
| 13 × `src/locales/*/profiles.json` | +Smart 配置字段标签         |

---

## 验证命令

```bash
# TypeScript 类型检查
pnpm typecheck

# ESLint (0 warnings)
pnpm lint

# Rust 编译 + Clippy
cargo clippy --all-targets --all-features -- -D warnings

# i18n 完整性检查 (0 missing, 0 unused)
pnpm i18n:check

# 重新生成 i18n 类型（修改翻译文件后必须运行）
node scripts/generate-i18n-keys.mjs
npx prettier --write src/types/generated/i18n-keys.ts src/types/generated/i18n-resources.ts
```
