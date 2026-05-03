## v1.7.7-legacy.15

### Notice

- `release/1.x-legacy` 维护线第十五个补丁版本
- 主题：根治"关闭后从托盘恢复"出现的边框黑线闪烁、启动慢、代理页节点列表抖动三类回归。这三类症状共享一个根因——之前所有"关闭"路径（标题栏 X、ESC、`open_or_close_dashboard` 快捷键）都走 `window.close()` 销毁 WebView2，托盘恢复要完整重建窗口与前端，等价于每次冷启动；本次拦截 `CloseRequested` 改为 hide，托盘恢复走 `unminimize/show/set_focus` 早返回分支，再叠加四处放大器修复
- **修订重发**：初版白闪修复在 `src/index.html` 注入 inline `<style>`，触发 Tauri 1.x 的 CSP nonce 注入流程，最终 CSP 退化为 `style-src 'self' 'nonce-...'`（按规范覆盖 `'unsafe-inline'`），导致 emotion 运行时注入的 MUI 样式（图标尺寸 / 字体 / 布局）全部被阻断，安装后整页 UI 错乱。本次将首帧底色移至 `src/assets/styles/index.scss` 走外链 CSS，Vite 把它产到 `<head>` 的 `<link>`（paint 前同步加载），既避开 nonce 模式又保留首帧上色效果

### Bugs Fixes

- **托盘恢复时窗口边框出现黑线闪烁**：Tauri 在 Windows 上用 `transparent + decorations(false) + window-shadows`，DWM 在客户区外画一圈暗色 frame；React/MUI 挂载前 body 是浏览器默认白底，与暗框对比就是观感上的"边框黑线 + 闪一下"。`index.scss` 顶部加首帧底色（`prefers-color-scheme`：dark `#2E303D` / light `#F5F5F5`），React 挂载前外链 CSS 已同步加载完成，DWM frame 不再有白色对比；同时 `use-custom-theme.ts` 在 `palette.background` 补 `default` 字段（之前只设 `paper`），防止 MUI 默认 `#fff` 在挂载后某些子组件露白（同源于 legacy.13 的 DataGrid 修复）
- **托盘恢复后启动慢可感知**：`main.rs` 的 `CloseRequested` 之前只存窗口尺寸位置不阻止销毁，每次"关闭 → 托盘恢复"都重建 WebView2 + 前端首屏。改为 `api.prevent_close() + window.hide()`，恢复路径走 `resolve::create_window` 早返回分支（`unminimize/show/set_focus`），不再重建。Quit 路径仍走 `cmds::exit_app` 的 `std::process::exit(0)`，不受影响
- **托盘恢复 / 切回应用瞬间 IPC 风暴**：SWRConfig 默认 `revalidateOnFocus:true`，focus 时整批 `useSWR(getProxies/getRules/getProxyProviders/getClashConfig/...)` 同帧并发 invoke，Tauri IPC cold-jitter 叠加形成"启动后卡一下"。`_layout.tsx` SWRConfig 加 `revalidateOnFocus:false`，各 hook 的定时刷新和事件驱动 `mutate` 仍是兜底，不影响数据新鲜度
- **代理页节点列表打开时抖动**（三处协同来源）：
  - `use-head-state.ts`：`useEffect` 异步 hydrate localStorage 折叠状态，首帧所有 group 走 `DEFAULT_STATE.open=false` 仅 group 头（~56px），下帧爆开到完整列表，Virtuoso 整列重测。改为 `useLayoutEffect` 在 paint 前完成 hydrate
  - `use-window-width.ts`：仅监听 `window.resize` 一次性读 `clientWidth`，托盘恢复 / WebView2 重新激活初期 resize 可能漏触发，导致 col 计算读到 0 → 单列布局，随后真正 resize 跳到 3/4 列，整列行类型从 type=2 切到 type=4 全部重测。补 `ResizeObserver(document.body)` 兜底
  - `proxy-render.tsx`：group 头 `<img width=32>` 缺 `height`，缓存图二次 onload 时由 intrinsic 决定行高，型号 0 行高度先后两次结算引发抖动。补 `height=32`
- **节点选中态横向 3px 位移引发可见抖动**：`ProxyItem` / `ProxyItemMini` 的 `&.Mui-selected` 之前用 `width: calc(100% + 3px); marginLeft: -3px; borderLeft: 3px solid;` 实现左边条，会让选中行真实宽度变化、整行水平 3px 位移；`group.now` 异步到达 / selected 切换瞬间会触发可见的横向抖动。改为 `boxShadow: inset 3px 0 0 selectColor`，视觉一致但不占布局

---

## v1.7.7-legacy.14

### Notice

- `release/1.x-legacy` 维护线第十四个补丁版本
- 集中收紧 .8 → .13 修复链遗留的 7 处隐患：清空逻辑、退出阻塞、重试窗口、导入硬化、配置降级提示、主题适配

### Bugs Fixes

- **杂项设置里清空"自定义延迟测试 URL/超时"无效**：`misc-viewer.tsx` `onSave` 之前发 `value || undefined`，配合后端 `patch!` 宏（None 跳过赋值）等于"用户清空 → 旧值锁死，placeholder 永远显示不出来"。改为直接发空字符串/0，`proxy-head.tsx` / `proxy-item*.tsx` 已有 `|| 默认` 兜底
- **关闭应用时 mihomo 卡死会阻塞退出最长 30s**：`stop_core` 内的 disable-tun PATCH 共用了 `put_configs` 的 30s 超时常量。拆成 `PUT_CONFIG_TIMEOUT = 30s`（hot reload）/ `PATCH_CONFIG_TIMEOUT = 3s`（控制类），退出最长 ≤ 3s
- **导入新订阅切错到已有的 remote**：`onImport` 旧实现 `find(type==="remote")` 取**第一个** remote，已有订阅时会切到老的而不是刚导入的。改为取最后一个（`append_item` push 末尾）
- **`onImport` 链路裸 unhandled rejection**：legacy.12 硬化了 `onSelect`，但 `onImport` 内的同等流程被遗漏——`activateSelected()` 未 await 未 try/catch。补齐 `useLockFn` / `setActivatings` spinner / 失败回滚 / try-catch
- **selector 偏好恢复重试窗口与注释矛盾**：`activateSelected` 仅 2×150ms = 300ms，但 .12 注释明确写"reload 实际可能 5-20s"——rule provider 多的用户切换后所有 selector 静默回到默认。改为 10×300ms（~3s 上限）
- **`activateSelected` 触发多余 `getProxies` fetch**：`mutate("getProxies", getProxies())` 第二参传了 promise 但 SWR 仍会 revalidate，相当于跑两次。改为单参 `mutate("getProxies")`
- **`newSelected` 落盘 `now: undefined`**：新出现的 Selector group 没有历史偏好时，旧逻辑会写 `{name, now: undefined}` 到 verge profile。改为用当前 `now` 兜底

### Performance / Tweaks

- **坏配置不再静默降级运行**：legacy.9 跳过 `check_config` 后，mihomo PUT 对部分软错误（rule provider URL 不可达、proxy 解析失败但其他可用）返回 204 但运行配置实际是降级版。把 `check_config` 改为 `spawn_blocking` 后台并行跑，失败时通过 `notice_message("config_validate::warn")` 弹 Notice.info（5s），不阻塞热路径。`_layout.tsx` 加对应监听 case
- **连接表背景接 theme palette**：`connection-table.tsx` 把写死的 `#282A36` / `#ffffff` 改用 `theme.palette.background.paper`，自定义主题色 / 跟随系统时与 BasePage 等其他面板背景对齐

---

## v1.7.7-legacy.13

### Notice

- `release/1.x-legacy` 维护线第十三个补丁版本
- 修复连接页路由切换回来时短暂的"白色一块再出现文字"的视觉闪烁

### Bugs Fixes

- 连接页 MUI X DataGrid 容器/列头/虚拟滚动区/overlay 的背景未跟随主题:`use-custom-theme.ts` 只设了 `palette.background.paper`,而 DataGrid 这些区域用的是 `palette.background.default`(MUI 出厂默认 `#fff`)。叠加 `_layout.tsx` 用 `TransitionGroup + key=pathname` 会让切回连接页时整页重挂载——重挂载第一帧 DataGrid 露出白底,WebSocket 数据到位后行的 `!important` 背景才覆盖,呈现"一块白色 → 立马出现文字"的视觉闪烁
- 修法:在 `connection-table.tsx` 的 DataGrid `sx` 中显式覆盖 `.MuiDataGrid-main / .MuiDataGrid-columnHeaders / .MuiDataGrid-virtualScroller / .MuiDataGrid-overlayWrapper` 与 root 容器的 `bgcolor`,沿用同一份基于主题的 `backgroundColor`。dark mode 下闪烁消除,light mode 无视觉变化

---

## v1.7.7-legacy.12

### Notice

- `release/1.x-legacy` 维护线第十二个补丁版本
- 围绕 legacy.9 的切换提速做防御性加固：竞态、超时、静默失败一并堵上

### Bugs Fixes

- 切换订阅时连点 A → B → A 不再污染 selector 偏好：`activateSelected` 纳入 `useLockFn` 锁内，`updateProxy` / `patchProfile` 改为 `await Promise.all` 后串行落盘，避免上一次切换的尾部写入覆盖当前 current 的偏好

### Performance

- 后端 `put_configs` / `patch_configs` 加客户端 30s timeout (`CONFIG_REQUEST_TIMEOUT`)：mihomo 卡死时单次 PUT 不再无限等待。30s 是覆盖最坏情况的安全值——hot reload 期间要重新加载所有 rule provider / external UI，rule provider 多 + 网络抖动时实际可能 5-20s
- `update_config` 取消外层重试，单次 PUT 走到底：之前的 5 次重试在 reload 慢的场景下会反复打断 mihomo，rule provider 较多 / 部分 URL 失效时 mihomo 永远完不成 reload（实测 17 分钟内被反复 reload 30+ 次）
- 切换成功的 Notice 提前到 `patchProfiles` 完成时立即弹出，主观提速 ~150ms；selector 偏好恢复改为后台串行任务

### Tweaks

- `onSelect` 切换期间设置 `activatings` 状态，复用 `profile-item.tsx` 已有的 spinner 蒙层组件，让"卡片已切 + 后台仍在工作"的过渡有视觉反馈，消除"切了好像没切"的不安

---

## v1.7.7-legacy.11

### Notice

- `release/1.x-legacy` 维护线第十一个补丁版本
- 维护性版本：编辑器 schema 跟进 mihomo 字段更新，同步整理 CI 配置

### Tweaks

- 配置编辑器 JSON Schema (`meta-json-schema`) 由 1.18.6 升至 1.19.24，跟上 mihomo 近一年的字段变更：`mode: Rule`、`sniffer.skip-domain` 含空格等 mihomo 实际接受的写法不再被编辑器误标红
- 纯前端 schema 校验包升级，不影响后端运行时；schema 文件路径未变

### Chore

- `.github/workflows/` 五个工作流（alpha / dev / release / release-1x-legacy / updater）整体回退到 legacy.7 时已知稳定的形态，撤销中间围绕 Node 24 升级、pnpm 安装顺序、release 上传权限的几次尝试性改动，CI 行为与 legacy.7 一致

---

## v1.7.7-legacy.10

### Notice

- `release/1.x-legacy` 维护线第十个补丁版本
- 默认端口对齐 clash classic 经典约定，方便复用既有客户端配置

### Tweaks

- 默认端口由 Verge 自定义的 7895-7899/9097 改为 clash classic 经典约定：
  - `mixed-port`: 7890（HTTP+SOCKS 混合，最常用）
  - `socks-port`: 7891
  - `port`（HTTP）: 7892（错开避免与 mixed-port 冲突）
  - `redir-port`: 7893
  - `tproxy-port`: 7894
  - `external-controller`: `127.0.0.1:9090`
- 仅影响**新装**或配置中相应字段缺失的情况；老用户已存在的 `clash-verge.yaml` 不会被覆盖
- 托盘"更多"子菜单的 `重启 Clash` 改名为 `重启内核`（实际跑的是 verge-mihomo，称谓更准确）

---

## v1.7.7-legacy.9

### Notice

- `release/1.x-legacy` 维护线第九个补丁版本
- 聚焦订阅切换响应速度，回归 v1.7.6 的快速链式刷新风格

### Performance

- 切换/导入订阅去掉 `setTimeout(1500/2000)` 固定等待，`await patchProfiles` 完成后立即 `mutate("getProxies")` 主动触发代理页 revalidate，不再等 layout 事件兜底
- `activateSelected` 改回链式调用（与 v1.7.6 一致），同时新增 2 次 × 150ms 轻量重试，容忍 mihomo reload 的边缘窗口期
- 后端 `update_config` 跳过 `check_config` 子进程校验：fork sidecar 跑 `-t` 是切换热路径单一最大延迟来源（300-1500ms），改由 `put_configs` 的 4xx 响应兜底，mihomo 当前运行配置不会被破坏
- 综合下来，切换感知从 ~1.8-3s 降至 ~0.2-0.6s
- 保留 legacy.6 引入的卡片乐观高亮与失败回滚

---

## v1.7.7-legacy.8

### Notice

- `release/1.x-legacy` 维护线第八个补丁版本
- 调整杂项设置(Miscellaneous)默认值，新装即可获得更合理的初始体验

### Tweaks

- `Default Latency Test`：未配置时前端兜底为 `http://www.gstatic.com/generate_204`（弹窗输入框直接显示该 URL）
- `Default Latency Timeout`：未配置时兜底为 `2000ms`（输入框 placeholder 同步更新）
- `Auto Log Clean`：首次安装写入 `verge.yaml` 的默认值由 `保留 90 天` 改为 `保留 7 天`
- 兜底逻辑仅修改前端显示与 `template()` 中的 `auto_log_clean`，不再向 `verge.yaml` 预写延迟测试相关字段，保持配置精简

---

## v1.7.7-legacy.7

### Notice

- `release/1.x-legacy` 维护线第七个补丁版本
- 撤销 legacy.2 / legacy.3 引入的"延迟测试默认值预填",回到 v1.7.7 原始行为

### Bugs Fixes

- 杂项设置中"默认测试链接"和"测试超时时间"恢复 v1.7.7 默认:输入框留空,只用 `http://1.1.1.1` 与 `10000ms` 作为 placeholder 提示,由用户按需填写
- 后端 `IVerge::template()` 不再向 `verge.yaml` 预写 `default_latency_test` / `default_latency_timeout`,新装用户初始配置文件保持精简
- 内部测速调用(`getProxyDelay` / `getGroupProxyDelays` / `clash_api::get_proxy_delay`)的兜底地址回到 `http://1.1.1.1`,不再强制走 `gstatic`

---

## v1.7.7-legacy.6

### Notice

- `release/1.x-legacy` 维护线第六个补丁版本
- 聚焦订阅切换的即时反馈与失败回滚

### Bugs Fixes

- 切换订阅时改为前端乐观更新，卡片瞬间高亮，不再叠加 loading 蒙层造成"已切换却仍在加载"的矛盾感
- 后端 `patchProfiles` 失败时自动回滚到原选中项，避免界面与运行配置不一致
- 节点选择恢复 `activateSelected` 改为后台异步执行，不阻塞切换主流程
- 导入新远程订阅自动选中时同样应用乐观更新

---

## v1.7.7-legacy.5

### Notice

- `release/1.x-legacy` 维护线第五个补丁版本
- 聚焦修复连接页速度排序显示不符合直觉的问题

### Bugs Fixes

- 表格视图中下载/上传速度与流量列点击排序时优先按从大到小排列
- 列表视图排序前复制连接数组，避免刷新连接数据时污染上一帧顺序

---

## v1.7.7-legacy.4

### Notice

- `release/1.x-legacy` 维护线第四个补丁版本
- 聚焦修复安装版 Service Mode / TUN 模式与新 service 二进制不兼容的问题

### Bugs Fixes

- Legacy 打包流程不再下载最新 `clash-verge-service` 产物
- Windows legacy 构建改为固定复用 `v1.7.7` portable 包内的兼容 service 三件套
- 避免安装版显示服务安装成功后，TUN 模式仍反复提示安装服务

---

## v1.7.7-legacy.3

### Notice

- `release/1.x-legacy` 维护线第三个补丁版本
- 聚焦代理页延迟测试默认行为调整，不涉及测试页面逻辑

### Bugs Fixes

- 将代理页默认测速超时时间从 `10000ms` 下调为 `2000ms`
- 将默认测速链接切换为 `https://www.gstatic.com/generate_204`
- 对于没有组内测速链接的代理组，未手动指定时统一回退到 `gstatic` 默认测速地址

---

## v1.7.7-legacy.2

### Notice

- `release/1.x-legacy` 维护线第二个补丁版本
- 基于 `v1.7.7-legacy.1` 继续修正本地打包链路和订阅页入口

### Bugs Fixes

- 修复本地打包不会自动准备 sidecar 和 resources，降低产物残缺概率
- 为本地前端生产构建补充更高的 Node 堆内存上限，避免构建阶段内存溢出
- 在打包前增加运行时资源缺失校验，缺文件时直接失败而不是生成有缺陷的安装包
- 收紧订阅页入口，移除全局扩展配置和全局扩展脚本卡片

---

## v1.7.7-legacy.1

### Notice

- `release/1.x-legacy` 独立维护线首个发布版本
- 基于原始 `Release 1.7.7` 代码状态重新打包
- GitHub Actions 发版、安装包和 updater 资产切换到独立 legacy 流程

### Features

- 新增专用 `1.x-legacy` GitHub Release 工作流
- Legacy 版本使用独立的 `updater-legacy` 更新通道
- 支持覆盖已有 `v1.7.7-legacy.1` tag/release 重新发版

### Notes

- 此版本目标是恢复并固定 `1.x` 维护线，不引入新的业务功能变更

---

## v1.7.7

### Bugs Fixes

- 修复导入订阅没有自动重载(不显示节点)的问题
- 英语状态下修复 Windows 工具栏提示文本超过限制的问题

---

## v1.7.6

### Notice

- Clash Verge Rev 目前已进入稳定周期，日后更新将着重于 bug 修复与内核常规升级

### Features

- Meta(mihomo)内核升级 1.18.7
- 界面细节调整
- 优化服务模式安装逻辑
- 移除无用的 console log
- 能自动选择第一个订阅

### Bugs Fixes

- 修复服务模式安装问题
- 修复 Mac 下的代理绕过 CIDR 写法过滤
- 修复 32 位升级 URL
- 修复不同分组 URL 测试地址配置无效的问题
- 修复 Web UI 下的一处 hostname 参数

---

## v1.7.5

### Features

- 展示局域网 IP 地址信息
- 在设置页面直接复制环境变量
- 优化服务模式安装逻辑

### Performance

- 优化切换订阅速度
- 优化更改端口速度

### Bugs Fixes

- 调整 MacOS 托盘图标大小
- Trojan URI 解析错误
- 卡片拖动显示层级错误
- 代理绕过格式检查错误
- MacOS 下编辑器最大化失败
- MacOS 服务安装失败
- 更改窗口大小导致闪退的问题

---

## v1.7.3

### Features

- 支持可视化编辑订阅代理组
- 支持可视化编辑订阅节点
- 支持可视化编辑订阅规则
- 扩展脚本支持订阅名称参数 `function main(config, profileName)`

### Bugs Fixes

- 代理绕过格式检查错误

---

## v1.7.2

### Break Changes

- 更新后请务必重新导入所有订阅，包括 Remote 和 Local
- 此版本重构了 Merge/Script，更新前请先备份好自定义 Merge 和 Script（更新并不会删除配置文件，但是旧版 Merge 和 Script 在更新后无法从前端访问，备份以防万一）
- Merge 改名为 `扩展配置`，分为 `全局扩展配置` 和 `订阅扩展配置`，全局扩展配置对所有订阅生效，订阅扩展配置只对关联的订阅生效
- Script 改名为 `扩展脚本`，同样分为 `全局扩展脚本` 和 `订阅扩展脚本`
- 订阅扩展配置在订阅右键菜单里进入
- 执行优先级为： 全局扩展配置 -> 全局扩展脚本 -> 订阅扩展配置 ->订阅扩展脚本
- 扩展配置删除了 `prepend/append` 能力，请使用 右键订阅 -> `编辑规则`/`编辑节点`/`编辑代理组` 来代替
- MacOS 用户更新后请重新安装服务模式

### Features

- 升级内核到 1.18.6
- 移除内核授权，改为服务模式实现
- 自动填充本地订阅名称
- 添加重大更新处理逻辑
- 订阅单独指定扩展配置/脚本（需要重新导入订阅）
- 添加可视化规则编辑器（需要重新导入订阅）
- 编辑器新增工具栏按钮（格式化、最大化/最小化）
- WEBUI 使用最新版 metacubex，并解决无法自动登陆问问题
- 禁用部分 Webview2 快捷键
- 热键配置新增连接符 + 号
- 新增部分悬浮提示按钮，用于解释说明
- 当日志等级为`Debug`时（更改需重启软件生效），支持点击内存主动内存回收（绿色文字）
- 设置页面右上角新增 TG 频道链接
- 各种细节优化和界面性能优化

### Bugs Fixes

- 修复代理绕过格式检查
- 通过进程名称关闭进程
- 退出软件时恢复 DNS 设置
- 修复创建本地订阅时更新间隔无法保存
- 连接页面列宽无法调整

---

## v1.7.1

### Break Changes

- 更新后请务必重新导入所有订阅，包括 Remote 和 Local
- 此版本重构了 Merge/Script，更新前请先备份好自定义 Merge 和 Script（更新并不会删除配置文件，但是旧版 Merge 和 Script 在更新后无法从前端访问，备份以防万一）
- Merge 改名为 `扩展配置`，分为 `全局扩展配置` 和 `订阅扩展配置`，全局扩展配置对所有订阅生效，订阅扩展配置只对关联的订阅生效
- Script 改名为 `扩展脚本`，同样分为 `全局扩展脚本` 和 `订阅扩展脚本`
- 订阅扩展配置在订阅右键菜单里进入
- 执行优先级为： 全局扩展配置 -> 全局扩展脚本 -> 订阅扩展配置 ->订阅扩展脚本
- 扩展配置删除了 `prepend/append` 能力，请使用 右键订阅 -> `编辑规则`/`编辑节点`/`编辑代理组` 来代替
- MacOS 用户更新后请重新安装服务模式

### Features

- 升级内核到 1.18.6
- 移除内核授权，改为服务模式实现
- 自动填充本地订阅名称
- 添加重大更新处理逻辑
- 订阅单独指定扩展配置/脚本（需要重新导入订阅）
- 添加可视化规则编辑器（需要重新导入订阅）
- 编辑器新增工具栏按钮（格式化、最大化/最小化）
- WEBUI 使用最新版 metacubex，并解决无法自动登陆问问题
- 禁用部分 Webview2 快捷键
- 热键配置新增连接符 + 号
- 新增部分悬浮提示按钮，用于解释说明
- 当日志等级为`Debug`时（更改需重启软件生效），支持点击内存主动内存回收（绿色文字）
- 设置页面右上角新增 TG 频道链接
- 各种细节优化和界面性能优化

### Bugs Fixes

- 修复代理绕过格式检查
- 通过进程名称关闭进程
- 退出软件时恢复 DNS 设置
- 修复创建本地订阅时更新间隔无法保存
- 连接页面列宽无法调整

---

## v1.7.0

### Break Changes

- 此版本重构了 Merge/Script，更新前请先备份好自定义 Merge 和 Script（更新并不会删除配置文件，但是旧版 Merge 和 Script 在更新后无法从前端访问，备份以防万一）
- Merge 改名为 `扩展配置`，分为 `全局扩展配置` 和 `订阅扩展配置`，全局扩展配置对所有订阅生效，订阅扩展配置只对关联的订阅生效
- Script 改名为 `扩展脚本`，同样分为 `全局扩展脚本` 和 `订阅扩展脚本`
- 执行优先级为： 全局扩展配置 -> 全局扩展脚本 -> 订阅扩展配置 ->订阅扩展脚本
- MacOS 用户更新后请重新安装服务模式

### Features

- 移除内核授权，改为服务模式实现
- 自动填充本地订阅名称
- 添加重大更新处理逻辑
- 订阅单独指定扩展配置/脚本（需要重新导入订阅）
- 添加可视化规则编辑器（需要重新导入订阅）
- 编辑器新增工具栏按钮（格式化、最大化/最小化）
- WEBUI 使用最新版 metacubex，并解决无法自动登陆问问题
- 禁用部分 Webview2 快捷键
- 热键配置新增连接符 + 号
- 新增部分悬浮提示按钮，用于解释说明
- 当日志等级为`Debug`时（更改需重启软件生效），支持点击内存主动内存回收（绿色文字）
- 设置页面右上角新增 TG 频道链接

### Bugs Fixes

- 修复代理绕过格式检查
- 通过进程名称关闭进程
- 退出软件时恢复 DNS 设置
- 修复创建本地订阅时更新间隔无法保存
- 连接页面列宽无法调整

---

## v1.6.6

### Features

- MacOS 应用签名
- 删除 AppImage
- 应用更新对话框添加下载按钮
- 设置系统代理绕过时保留默认值
- 系统代理绕过设置输入格式检查

### Bugs Fixes

- MacOS 代理组图标无法显示
- RPM 包依赖缺失

---

## v1.6.5

### Features

- 添加 RPM 包支持
- 优化细节

### Bugs Fixes

- MacOS 10.15 编辑器空白的问题
- MacOS 低版本启动白屏的问题

---

## v1.6.4

### Features

- 系统代理支持 PAC 模式
- 允许关闭不使用的端口
- 使用新的应用图标
- MacOS 支持切换托盘图标单色/彩色模式
- CSS 注入支持通过编辑器编辑
- 优化代理组列表性能
- 优化流量图显性能
- 支持波斯语

### Bugs Fixes

- Kill 内核后 Tun 开启缓慢的问题
- 代理绕过为空时使用默认值
- 无法读取剪切板内容
- Windows 下覆盖安装无法内核占用问题

---

## v1.6.2

### Features

- 支持本地文件拖拽导入
- 重新支持 32 位 CPU
- 新增内置 Webview2 版本
- 优化 Merge 逻辑，支持深度合并
- 删除 Merge 配置中的 append/prepend-provider 字段
- 支持更新稳定版内核

### Bugs Fixes

- MacOS DNS 还原失败
- CMD 环境变量格式错误
- Linux 下与 N 卡的兼容性问题
- 修改 Tun 设置不立即生效

---

## v1.6.1

### Features

- 鼠标悬浮显示当前订阅的名称 [#938](https://github.com/clash-verge-rev/clash-verge-rev/pull/938)
- 日志过滤支持正则表达式 [#959](https://github.com/clash-verge-rev/clash-verge-rev/pull/959)
- 更新 Clash 内核到 1.18.4

### Bugs Fixes

- 修复 Linux KDE 环境下系统代理无法开启的问题
- 窗口最大化图标调整 [#924](https://github.com/clash-verge-rev/clash-verge-rev/pull/924)
- 修改 MacOS 托盘点击行为(左键菜单，右键点击事件)
- 修复 MacOS 服务模式安装失败的问题

---

## v1.6.0

### Features

- Meta(mihomo)内核回退 1.18.1（当前新版内核 hy2 协议有 bug，等修复后更新）
- 多处界面细节调整 [#724](https://github.com/clash-verge-rev/clash-verge-rev/pull/724) [#799](https://github.com/clash-verge-rev/clash-verge-rev/pull/799) [#900](https://github.com/clash-verge-rev/clash-verge-rev/pull/900) [#901](https://github.com/clash-verge-rev/clash-verge-rev/pull/901)
- Linux 下新增服务模式
- 新增订阅卡片右键可以打开机场首页
- url-test 支持手动选择、节点组 fixed 节点使用角标展示 [#840](https://github.com/clash-verge-rev/clash-verge-rev/pull/840)
- Clash 配置、Merge 配置提供 JSON Schema 语法支持、连接界面调整 [#887](https://github.com/clash-verge-rev/clash-verge-rev/pull/887)
- 修改 Merge 配置文件默认内容 [#889](https://github.com/clash-verge-rev/clash-verge-rev/pull/889)
- 修改 tun 模式默认 mtu 为 1500，老版本升级，需在 tun 模式设置下“重置为默认值”。
- 使用 npm 安装 meta-json-schema [#895](https://github.com/clash-verge-rev/clash-verge-rev/pull/895)
- 更新部分翻译 [#904](https://github.com/clash-verge-rev/clash-verge-rev/pull/904)
- 支持 ico 格式的任务栏图标

### Bugs Fixes

- 修复 Linux KDE 环境下系统代理无法开启的问题
- 修复延迟检测动画问题
- 窗口最大化图标调整 [#816](https://github.com/clash-verge-rev/clash-verge-rev/pull/816)
- 修复 Windows 某些情况下无法安装服务模式 [#822](https://github.com/clash-verge-rev/clash-verge-rev/pull/822)
- UI 细节修复 [#821](https://github.com/clash-verge-rev/clash-verge-rev/pull/821)
- 修复使用默认编辑器打开配置文件
- 修复内核文件在特定目录也可以更新的问题 [#857](https://github.com/clash-verge-rev/clash-verge-rev/pull/857)
- 修复服务模式的安装目录问题
- 修复删除配置文件的“更新间隔”出现的问题 [#907](https://github.com/clash-verge-rev/clash-verge-rev/issues/907)

### 已知问题（历史遗留问题，暂未找到有效解决方案）

- MacOS M 芯片下服务模式无法安装；临时解决方案：在内核 ⚙️ 下，手动授权，再打开 tun 模式。
- MacOS 下如果删除过网络配置，会导致无法正常打开系统代理；临时解决方案：使用浏览器代理插件或手动配置系统代理。
- Window 拨号连接下无法正确识别并打开系统代理；临时解决方案：使用浏览器代理插件或使用 tun 模式。

---

## v1.5.11

### Features

- Meta(mihomo)内核更新 1.18.2

### Bugs Fixes

- 升级图标无法点击的问题
- 卸载时检查安装目录是否为空
- 代理界面图标重合的问题

---

## v1.5.10

### Features

- 优化 Linux 托盘菜单显示
- 添加透明代理端口设置
- 删除订阅前确认

### Bugs Fixes

- 删除 MacOS 程序坞图标
- Windows 下 service 日志没有清理
- MacOS 无法开启系统代理

---

## v1.5.9

### Features

- 缓存代理组图标
- 使用`boa_engine` 代替 `rquickjs`
- 支持 Linux armv7

### Bugs Fixes

- Windows 首次安装无法点击
- Windows 触摸屏无法拖动
- 规则列表 `REJECT-DROP` 颜色
- MacOS Dock 栏不显示图标
- MacOS 自定义字体无效
- 避免使用空 UA 拉取订阅

---

## v1.5.8

### Features

- 优化 UI 细节
- Linux 绘制窗口圆角
- 开放 DevTools

### Bugs Fixes

- 修复 MacOS 下开启 Tun 内核崩溃的问题

---

## v1.5.7

### Features

- 优化 UI 各种细节
- 提供菜单栏图标样式切换选项(单色/彩色/禁用)
- 添加自动检查更新开关
- MacOS 开启 Tun 模式自动修改 DNS
- 调整可拖动区域(尝试修复触摸屏无法拖动的问题)

---

## v1.5.6

### Features

- 全新专属 Verge rev UI 界面 (by @Amnesiash) 及细节调整
- 提供允许无效证书的开关
- 删除不必要的快捷键
- Provider 更新添加动画
- Merge 支持 Provider
- 更换订阅框的粘贴按钮，删除默认的"Remote File" Profile 名称
- 链接菜单添加节点显示

### Bugs Fixes

- Linux 下图片显示错误

---

## v1.5.4

### Features

- 支持自定义托盘图标
- 支持禁用代理组图标
- 代理组显示当前代理
- 修改 `打开面板` 快捷键为`打开/关闭面板`

---

## v1.5.3

### Features

- Tun 设置添加重置按钮

### Bugs Fixes

- Tun 设置项显示错误的问题
- 修改一些默认值
- 启动时不更改启动项设置

---

## v1.5.2

### Features

- 支持自定义延迟测试超时时间
- 优化 Tun 相关设置

### Bugs Fixes

- Merge 操作出错
- 安装后重启服务
- 修复管理员权限启动时开机启动失效的问题

---

## v1.5.1

### Features

- 保存窗口最大化状态
- Proxy Provider 显示数量
- 不再提供 32 位安装包（因为 32 位经常出现各种奇怪问题，比如 tun 模式无法开启；现在系统也几乎没有 32 位了）

### Bugs Fixes

- 优化设置项名称
- 自定义 GLOBAL 代理组时代理组显示错误的问题

---

## v1.5.0

### Features

- 删除 Clash 字段过滤功能
- 添加 socks 端口和 http 端口设置
- 升级内核到 1.18.1

### Bugs Fixes

- 修复 32 位版本无法显示流量信息的问题

---

## v1.4.11

### Break Changes

- 此版本更改了 Windows 安装包安装模式，需要卸载后手动安装，否则无法安装到正确位置

### Features

- 优化了系统代理开启的代码，解决了稀有场景下代理开启卡顿的问题
- 添加 MacOS 下的 debug 日志，以便日后调试稀有场景下 MacOS 下无法开启系统代理的问题
- MacOS 关闭 GUI 时同步杀除后台 GUI [#306](https://github.com/clash-verge-rev/clash-verge-rev/issues/306)

### Bugs Fixes

- 解决自动更新时文件占用问题
- 解决稀有场景下系统代理开启失败的问题
- 删除冗余内核代码

---

## v1.4.10

### Features

- 设置中添加退出按钮
- 支持自定义软件启动页
- 在 Proxy Provider 页面展示订阅信息
- 优化 Provider 支持

### Bugs Fixes

- 更改端口时立即重设系统代理
- 网站测试超时错误

---

## v1.4.9

### Features

- 支持启动时运行脚本
- 支持代理组显示图标
- 新增测试页面

### Bugs Fixes

- 连接页面时间排序错误
- 连接页面表格宽度优化

---

## v1.4.8

### Features

- 连接页面总流量显示

### Bugs Fixes

- 连接页面数据排序错误
- 新建订阅时设置更新间隔无效
- Windows 拨号网络无法设置系统代理
- Windows 开启/关闭系统代理延迟(使用注册表即可)
- 删除无效的背景模糊选项

---

## v1.4.7

### Features

- Windows 便携版禁用应用内更新
- 支持代理组 Hidden 选项
- 支持 URL Scheme(MacOS & Linux)

---

## v1.4.6

### Features

- 更新 Clash Meta(mihomo) 内核到 v1.18.0
- 支持 URL Scheme(暂时仅支持 Windows)
- 添加窗口置顶按钮
- UI 优化调整

### Bugs Fixes

- 修复一些编译错误
- 获取订阅名称错误
- 订阅信息解析错误

---

## v1.4.5

### Features

- 更新 MacOS 托盘图标样式(@gxx2778 贡献)

### Bugs Fixes

- Windows 下更新时无法覆盖`clash-verge-service.exe`的问题(需要卸载重装一次服务，下次更新生效)
- 窗口最大化按钮变化问题
- 窗口尺寸保存错误问题
- 复制环境变量类型无法切换问题
- 某些情况下闪退的问题
- 某些订阅无法导入的问题

---

## v1.4.4

### Features

- 支持 Windows aarch64(arm64) 版本
- 支持一键更新 GeoData
- 支持一键更新 Alpha 内核
- MacOS 支持在系统代理时显示不同的托盘图标
- Linux 支持在系统代理时显示不同的托盘图标
- 优化复制环境变量逻辑

### Bugs Fixes

- 修改 PID 文件的路径

### Performance

- 优化创建窗口的速度

---

## v1.4.3

### Break Changes

- 更改配置文件路径到标准目录(可以保证卸载时没有残留)
- 更改 appid 为 `io.github.clash-verge-rev.clash-verge-rev`
- 建议卸载旧版本后再安装新版本，该版本安装后不会使用旧版配置文件，你可以手动将旧版配置文件迁移到新版配置文件目录下

### Features

- 移除页面切换动画
- 更改 Tun 模式托盘图标颜色
- Portable 版本默认使用当前目录作为配置文件目录
- 禁用 Clash 字段过滤时隐藏 Clash 字段选项
- 优化拖拽时光标样式

### Bugs Fixes

- 修复 windows 下更新时没有关闭内核导致的更新失败的问题
- 修复打开文件报错的问题
- 修复 url 导入时无法获取中文配置名称的问题
- 修复 alpha 内核无法显示内存信息的问题

---

## v1.4.2

### Features

- update clash meta core to mihomo 1.17.0
- support both clash meta stable release and prerelease-alpha release
- fixed the problem of not being able to set the system proxy when there is a dial-up link on windows system [#833](https://github.com/zzzgydi/clash-verge/issues/833)
- support new clash field
- support random mixed port
- add windows x86 and linux armv7 support
- support disable tray click event
- add download progress for updater
- support drag to reorder the profile
- embed emoji fonts
- update depends
- improve UI style

---

## v1.4.1

### Features

- update clash meta core to newest 虚空终端(2023.11.23)
- delete clash core UI
- improve UI
- change Logo to original

---

## v1.4.0

### Features

- update clash meta core to newest 虚空终端
- delete clash core, no longer maintain
- merge Clash nyanpasu changes
- remove delay display different color
- use Meta Country.mmdb
- update dependencies
- small changes here and there

---

## v1.3.8

### Features

- update clash meta core
- add default valid keys
- adjust the delay display interval and color

### Bug Fixes

- fix connections page undefined exception

---

## v1.3.7

### Features

- update clash and clash meta core
- profiles page add paste button
- subscriptions url textfield use multi lines
- set min window size
- add check for updates buttons
- add open dashboard to the hotkey list

### Bug Fixes

- fix profiles page undefined exception

---

## v1.3.6

### Features

- add russian translation
- support to show connection detail
- support clash meta memory usage display
- support proxy provider update ui
- update geo data file from meta repo
- adjust setting page

### Bug Fixes

- center the window when it is out of screen
- use `sudo` when `pkexec` not found (Linux)
- reconnect websocket when window focus

### Notes

- The current version of the Linux installation package is built by Ubuntu 20.04 (Github Action).

---

## v1.3.5

### Features

- update clash core

### Bug Fixes

- fix blurry system tray icon (Windows)
- fix v1.3.4 wintun.dll not found (Windows)
- fix v1.3.4 clash core not found (macOS, Linux)

---

## v1.3.4

### Features

- update clash and clash meta core
- optimize traffic graph high CPU usage when window hidden
- use polkit to elevate permission (Linux)
- support app log level setting
- support copy environment variable
- overwrite resource file according to file modified
- save window size and position

### Bug Fixes

- remove fallback group select status
- enable context menu on editable element (Windows)

---

## v1.3.3

### Features

- update clash and clash meta core
- show tray icon variants in different system proxy status (Windows)
- close all connections when mode changed

### Bug Fixes

- encode controller secret into uri
- error boundary for each page

---

## v1.3.2

### Features

- update clash and clash meta core

### Bug Fixes

- fix import url issue
- fix profile undefined issue

---

## v1.3.1

### Features

- update clash and clash meta core

### Bug Fixes

- fix open url issue
- fix appimage path panic
- fix grant root permission in macOS
- fix linux system proxy default bypass

---

## v1.3.0

### Features

- update clash and clash meta
- support opening dir on tray
- support updating all profiles with one click
- support granting root permission to clash core(Linux, macOS)
- support enable/disable clash fields filter, feel free to experience the latest features of Clash Meta

### Bug Fixes

- deb add openssl depend(Linux)
- fix the AppImage auto launch path(Linux)
- fix get the default network service(macOS)
- remove the esc key listener in macOS, cmd+w instead(macOS)
- fix infinite retry when websocket error

---

## v1.2.3

### Features

- update clash
- adjust macOS window style
- profile supports UTF8 with BOM

### Bug Fixes

- fix selected proxy
- fix error log

---

## v1.2.2

### Features

- update clash meta
- recover clash core after panic
- use system window decorations(Linux)

### Bug Fixes

- flush system proxy settings(Windows)
- fix parse log panic
- fix ui bug

---

## v1.2.1

### Features

- update clash version
- proxy groups support multi columns
- optimize ui

### Bug Fixes

- fix ui websocket connection
- adjust delay check concurrency
- avoid setting login item repeatedly(macOS)

---

## v1.2.0

### Features

- update clash meta version
- support to change external-controller
- support to change default latency test URL
- close all connections when proxy changed or profile changed
- check the config by using the core
- increase the robustness of the program
- optimize windows service mode (need to reinstall)
- optimize ui

### Bug Fixes

- invalid hotkey cause panic
- invalid theme setting cause panic
- fix some other glitches

---

## v1.1.2

### Features

- the system tray follows i18n
- change the proxy group ui of global mode
- support to update profile with the system proxy/clash proxy
- check the remote profile more strictly

### Bug Fixes

- use app version as default user agent
- the clash not exit in service mode
- reset the system proxy when quit the app
- fix some other glitches

---

## v1.1.1

### Features

- optimize clash config feedback
- hide macOS dock icon
- use clash meta compatible version (Linux)

### Bug Fixes

- fix some other glitches

---

## v1.1.0

### Features

- add rule page
- supports proxy providers delay check
- add proxy delay check loading status
- supports hotkey/shortcut management
- supports displaying connections data in table layout(refer to yacd)

### Bug Fixes

- supports yaml merge key in clash config
- detect the network interface and set the system proxy(macOS)
- fix some other glitches

---

## v1.0.6

### Features

- update clash and clash.meta

### Bug Fixes

- only script profile display console
- automatic configuration update on demand at launch

---

## v1.0.5

### Features

- reimplement profile enhanced mode with quick-js
- optimize the runtime config generation process
- support web ui management
- support clash field management
- support viewing the runtime config
- adjust some pages style

### Bug Fixes

- fix silent start
- fix incorrectly reset system proxy on exit

---

## v1.0.4

### Features

- update clash core and clash meta version
- support switch clash mode on system tray
- theme mode support follows system

### Bug Fixes

- config load error on first use

---

## v1.0.3

### Features

- save some states such as URL test, filter, etc
- update clash core and clash-meta core
- new icon for macOS

---

## v1.0.2

### Features

- supports for switching clash core
- supports release UI processes
- supports script mode setting

### Bug Fixes

- fix service mode bug (Windows)

---

## v1.0.1

### Features

- adjust default theme settings
- reduce gpu usage of traffic graph when hidden
- supports more remote profile response header setting
- check remote profile data format when imported

### Bug Fixes

- service mode install and start issue (Windows)
- fix launch panic (Some Windows)

---

## v1.0.0

### Features

- update clash core
- optimize traffic graph animation
- supports interval update profiles
- supports service mode (Windows)

### Bug Fixes

- reset system proxy when exit from dock (macOS)
- adjust clash dns config process strategy

---

## v0.0.29

### Features

- sort proxy node
- custom proxy test url
- logs page filter
- connections page filter
- default user agent for subscription
- system tray add tun mode toggle
- enable to change the config dir (Windows only)

---

## v0.0.28

### Features

- enable to use clash config fields (UI)

### Bug Fixes

- remove the character
- fix some icon color

---

## v0.0.27

### Features

- supports custom theme color
- tun mode setting control the final config

### Bug Fixes

- fix transition flickers (macOS)
- reduce proxy page render

---

## v0.0.26

### Features

- silent start
- profile editor
- profile enhance mode supports more fields
- optimize profile enhance mode strategy

### Bug Fixes

- fix csp restriction on macOS
- window controllers on Linux

---

## v0.0.25

### Features

- update clash core version

### Bug Fixes

- app updater error
- display window controllers on Linux

### Notes

If you can't update the app properly, please consider downloading the latest version from github release.

---

## v0.0.24

### Features

- Connections page
- add wintun.dll (Windows)
- supports create local profile with selected file (Windows)
- system tray enable set system proxy

### Bug Fixes

- open dir error
- auto launch path (Windows)
- fix some clash config error
- reduce the impact of the enhanced mode

---

## v0.0.23

### Features

- i18n supports
- Remote profile User Agent supports

### Bug Fixes

- clash config file case ignore
- clash `external-controller` only port
