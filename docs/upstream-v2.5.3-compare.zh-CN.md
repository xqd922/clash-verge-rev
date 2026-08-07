# Upstream 对比审计：v2.5.2 → dev(2.5.3) 选择性采纳

审计日期：2026-08-07
基准：`personal` = `72cdaa4c`（release 7.0.35，含 `3fab8159` TUN 开机自启修复）
上游：`upstream/main` = `28f2efc5`（v2.5.2）、`upstream/dev` = `2f44d2bc`（2.5.3，较 v2.5.2 多 79 个提交）
共同基线：`a00c67e8`（上游 v2.5.1）

## 结论速览

- 上游 v2.5.2+ 在「服务/运行态」层做了大重构（`runstate` 模块、`mihomo ipc` 重构、
  `service-ipc` 升级、启动护栏与多用户服务所有权等）。这套架构更健壮，但依赖把
  `tauri-plugin-mihomo` 从本地 path 依赖切到上游 git 分支，与个人版长期决策冲突；
  **本次不整体引入**，只吸收其中与个人版直接相关的小改进。
- 本次已把 7 个「明确更好、且与 personal 架构兼容」的上游改动落到 `sync/upstream-better-20260807`，
  全部通过 `cargo check` / `clippy -D warnings` / 63 个单元测试 / biome 格式检查。
- 你的 `3fab8159`（等待服务 IPC 就绪再判断是否临时关闭 TUN）与上游 `993cc2ff`（v2.5.2 内）是
  同一问题的两个实现；本分支进一步把等待预算从 3~5 秒提升到 30 秒，覆盖「Windows 服务冷启动慢」的场景。

## 一、本次采纳（已实现）

| 上游提交 | 说明 | 落地方式 |
| --- | --- | --- |
| `95bd5cee` | js-yaml 默认导入改命名空间导入（ESM 兼容） | 5 个前端文件直接移植 |
| `85b5501f` | 代理编辑页空 name 节点导致 @dnd-kit 崩溃 | 前端直接移植 |
| `382cbf9f` | DNS 查看器空 YAML 解析报错 | 前端直接移植 |
| `6e928843` | Windows 下把 sidecar 挂到 Job Object，主程序退出时 sidecar 一并退出 | cherry-pick 并解决冲突（`windows-sys` 依赖 + `manager/mod.rs`/`state.rs`） |
| `1112f073`（部分） | Windows 服务冷启动等待从 3s 提升到 30s | `constants.rs` `SERVICE_WAIT_MAX` 3000→30000；`wait_for_service_available` 改用同一预算 |
| `49257a02` | 刷新代理配置事件导致 app 卡死 | `notification.rs` 新增 `RefreshProxyConfig` 事件 + `handle.rs` 助手 + `feat/profile.rs` 改用助手 |
| `6ad480f8`（部分） | 允许局域网时把回环 bind-address 放宽为 `*` | `enhance/mod.rs` 移植 `ensure_lan_bind_address` + 测试；`feat/config.rs` 部分 personal 已等价 |

## 二、与你 `3fab8159` 修复的关系（服务/TUN 生命周期）

- 上游 `993cc2ff`（v2.5.2，2026-07-15）就是 `should_wait_for_service` + 测试
  `service_wait_is_only_required_for_non_admin_tun`，与你的 `3fab8159` 实现一致；你在 v2.5.1 基础上
  重新实现了它，并额外覆盖了 `config.rs` 启动期误关 TUN 的路径。
- 上游后续 `1112f073` 把 `SERVICE_WAIT_MAX` 从 3s 提到 30s，并新增 sidecar→service 交接 watcher
  （回退 sidecar 后 120s 窗口内服务就绪再交接，让 TUN 最终能起来）。**交接 watcher 依赖上游一整套
  service.rs 重构（复杂 `ServiceManager` 结构），与 personal 的 v2.5.1 元组结构冲突大，本次未引入**；
  本次只吸收 30s 等待预算部分，已让「服务慢启动」场景下 TUN 不被误关。
- 建议后续：若想彻底解决「服务一直起不来时 TUN 仍能自启」，需要把上游 runstate/交接机制整体引入，
  属于独立的大工程（见第四节）。

## 三、逐项对比：已评估、未采纳

| 上游改动 | 为什么不采纳 |
| --- | --- |
| `c90de135` + `cefcb150` refactor/fix core runstate | 依赖 `tauri-plugin-mihomo` git 分支与 `service-ipc` 新版本，与个人本地 path 依赖决策冲突；改动量最大 |
| `b71a1e4e` refactor mihomo ipc | 同上，整个服务层 API 更换 |
| `23dda476` 启动护栏 + 多用户服务所有权 | 建立在 runstate 之上 |
| `7796259b` 自动 mixed port 回退 | 依赖新 config/runstate 机制；个人版改动面大 |
| `304f5bad` 代理数据合并为单个 IPC | 780 行新 `proxy_view.rs` + 前端大改，与个人 Smart 代理页冲突 |
| `b5766824` / `3d8d7731` 等 proxy 修复 | 建立在 `get_proxy_view` 等 v2.5.2 新结构上，personal 的 `cmd/proxy.rs` 没有这些代码 |
| `c5b641e4` profile 删除可回滚 | 改动 11 个文件含 lifecycle，风险高；可单独评估 |
| `ccf21f4a` 初始化时应用 runtime 配置 | 依赖上游 `IRuntime::apply()`（v2.5.2 后端重构产物），personal 没有该方法 |
| `fd70d4c6` GUI TUN 配置权威 | personal 的 enhance 顺序是「profile merge/script 在前、GUI 配置最后合并」，GUI 已天然权威，无需改动 |
| `1599ca0e` file-not-found 上下文 | 主体在重构后的 service.rs/state.rs，个人版仅能部分移植，收益一般 |
| `e7b1807f` 俄语翻译、CI/依赖升级类 | 与个人功能无关或由 lockfile 重生成覆盖 |

## 四、后续建议（如需继续）

1. **完整服务层同步**：在 sync 分支上把上游 dev 的 runstate/服务层合入 personal，并把
   `crates/tauri-plugin-mihomo` 升级到上游新 API（或接受 git 依赖）。这是解决
   「服务慢/挂时 TUN 开机自启」最彻底的方案，但需要 1~2 天级别的回归测试。
2. **profile 删除可回滚**（`c5b641e4`）：独立评估后可作为下一个单项采纳。
3. 每次同步前跑 `scripts/check-personal-ui-regression.mjs` 防止个人 UI 决策被覆盖。

## 验证记录

- `cargo check -p clash-verge`：通过
- `cargo clippy -p clash-verge --lib -- -D warnings`：通过
- `cargo test -p clash-verge --lib`：63 passed（含新增 `lan_bind_address_*`、`job_kills_child_on_handle_drop`）
- `pnpm typecheck`：仅剩 5 个 personal 分支既有错误（traffic/log 类型，与本次改动无关）
- `biome format`：改动文件全部合规
