## v2.5.5-8

<details>
<summary><strong> 🚀 优化改进 </strong></summary>

- 安装包附带自己的服务程序，服务模式下切换订阅时按 URL 保留已下载的规则集

</details>

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
## v2.5.6

<details>
<summary><strong> 🐞 修复问题 </strong></summary>

- 修复「代理集合」「规则集合」订阅响应较慢时更新误报失败、且不显示真实失败原因的问题

**🖥️ Windows**

- 修复 Windows 部分未安装服务的用户以普通权限运行时，内核无法启动的问题
- 修复 Windows 系统隔离权限被误判，导致服务模式和 TUN 无法使用的问题
- 修复 Windows 旧版服务状态残留导致无法重装的问题，自动备份可识别的残留并恢复安装
- 修复 Windows 服务已停止时，启动要等约两分钟才显示窗口的问题
- 优化 Windows 服务安全检查失败提示：用易懂的文字说明原因，并提供修复文档链接

**🍎 macOS**

- macOS 修复编辑规则时「规则内容」输入的首字母被自动转为大写的问题
- macOS 修复残留的服务进程导致内核无法启动、「继续使用 Sidecar」失败且无法修复的问题
- macOS 修复部分账户安装或修复服务时提示无法解析用户组而失败的问题
- macOS 优化用户目录在外置磁盘时服务拒绝启动内核的提示：说明原因并给出修复命令
- macOS 修复服务模式内核意外停止后，系统代理仍指向失效端口导致无法上网的问题

</details>

<details>
<summary><strong> 🚀 优化改进 </strong></summary>

- 优化内核启动失败和服务模式内核意外停止的提示：显示具体原因，并在窗口恢复后提示尚未解决的错误

</details>
