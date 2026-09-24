## v2.5.5-7

<details>
<summary><strong> 🐞 修复问题 </strong></summary>

- 修复检查更新仍指向官方仓库的问题，改为使用自己的发布地址
- 修复手动检查更新后不显示这次结果的问题

</details>

<details>
<summary><strong> 🚀 优化改进 </strong></summary>

- 服务模式下切换订阅时，按 URL 保留已下载的远程规则集，切回同一订阅不再重新下载

</details>

## v2.5.5-6

<details>
<summary><strong> 🐞 修复问题 </strong></summary>

- 修复超时和 Error 节点悬停时显示成「检测」的问题，继续显示 Timeout 或 Error
- 修复切换订阅时遮罩出现偏晚的问题，点击后立即覆盖
- 修复代理组标题下滑时看起来浮在列表上的问题

</details>

<details>
<summary><strong> 🚀 优化改进 </strong></summary>

- 新配置中「IPv6」和「统一延迟」默认关闭

</details>

## v2.5.5-5

<details>
<summary><strong> 🚀 优化改进 </strong></summary>

- 默认关闭「自动检查更新」，打开软件时不再自动检查

**🖥️/🍎 Windows/macOS**

- macOS 默认打开「优先使用系统标题栏」，Windows 默认关闭

</details>

## v2.5.5-4

<details>
<summary><strong> 🐞 修复问题 </strong></summary>

- 修复日志页每次打开都播放下拉动画的问题，改为直接定位到最新日志
- 修复虚拟网卡未就绪时由开关直接安装的问题，改回开关旁的安装按钮，装好后再打开
- 修复服务版本不符时从开关安装的问题，改到「设置」中重新安装

</details>

<details>
<summary><strong> 🚀 优化改进 </strong></summary>

- 未自定义延迟测试时，默认地址改为 `https://www.gstatic.com/generate_204`，超时改为 2000 毫秒

</details>

## v2.5.5-3

<details>
<summary><strong> 🐞 修复问题 </strong></summary>

- 修复代理组的延迟测试、定位、筛选和排序单独占一行的问题，改回标题左侧
- 修复连接页第一次点击速度、流量或时长时未按从大到小排序的问题
- 修复订阅页常见宽度下一排只有两张卡片的问题，恢复一排三个
- 修复冷启动时页面先下移再弹回的问题

</details>

<details>
<summary><strong> 🚀 优化改进 </strong></summary>

- 优化连接表表头不再换行，并加宽速度列和流量列
- 优化代理组展开状态，打开时直接使用上次的展开记录

</details>

## v2.5.5-2

<details>
<summary><strong> 🚀 优化改进 </strong></summary>

- 优化代理组筛选图标，改回漏斗样式
- 优化标题栏为系统原生按钮样式，并调整高度，避免更新提示挡住标题

</details>

## v2.5.5-1

<details>
<summary><strong> 🚀 优化改进 </strong></summary>

- 隐藏侧边栏首页，启动后直接进入代理页
- 隐藏订阅页的全局扩展和脚本覆写入口

</details>

## v2.5.5

<details>
<summary><strong> 🐞 修复问题 </strong></summary>

- 修复不同来源的订阅或规则集共用缓存路径时，服务模式无法启动内核的问题
- 优化文件错误日志，显示具体文件路径和失败原因
- 修复 DNS 覆写：空内容报错、初始值覆盖已有设置、已填写字段残留原配置、空字段在高级编辑器中缺失
- 修复 DNS 覆写中已填写的字段被 Merge 或 Script 覆盖的问题，空字段继续使用扩展配置
- 修复 Merge 覆写 DNS 策略、过滤条件和 hosts 时，旧条目仍被保留的问题
- 修复首次以管理员身份启动后，系统服务因数据目录所有权不符而拒绝启动内核的问题
- 修复重新安装服务后内核缺失、无法继续使用 Sidecar 的问题
- 修复内核未运行时，首页仍显示服务模式的问题
- 修复服务重装重复请求管理员授权、卸载后误报失败的问题

**🖥️ Windows**

- 修复 Windows 服务因路径或目录权限问题无法启动时，缺少重新安装服务的提示
- 修复 Windows 服务回退检查失败后状态未更新、无法重新尝试 Sidecar 的问题

**🐧 Linux**

- Linux 修复安装或修复服务后仍无法启动内核、开启 TUN 模式的问题

</details>

<details>
<summary><strong> 🚀 优化改进 </strong></summary>

- 优化扩展覆写配置和脚本编辑窗口，提示作用范围、执行顺序及应用设置优先级，并高亮当前步骤
- 优化 DNS 覆写开关：按订阅独立记忆，自动关闭仅影响当前订阅，并提示作用范围
- 优化系统服务兼容性检查：升级后服务版本或协议不兼容时，提示重新安装
- 优化预构建资源，补充 ASN 数据库文件
- 扩展配置或脚本写入由应用设置接管的字段时，弹出提示并在扩展日志中记录该值已被丢弃

</details>
