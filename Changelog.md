## v(7.0.1)

### 🔧 调整

- 统一延迟开关默认关闭

## v(7.0.0)

### ✨ 新功能

- 支持 Smart (mihomo-smart) 智能代理核心
- Smart 代理组自动转换：使用 Smart 核心时，自动将 url-test/fallback/load-balance 组转换为 smart 类型
- Smart 代理组兼容回退：切回标准核心时，自动将 smart 组恢复为 url-test
- 切换 Smart 核心时自动解除固定节点锁定
- Smart 组不再显示固定图标（由 ML 策略自动选择）

### 🐞 修复

- 修复 Smart 核心切换时配置生成错误
- 修复 mihomo 插件 build.rs 中缺少 Smart 命令注册
- 修复 Rust 缓存导致的 Smart ACL 权限错误

### 🚀 改进

- 移除首页，默认进入代理页面
- 移除全局扩展脚本 (Merge/Script)
- 基于 Clash Verge Rev 精简优化

## v(2.4.6)

> [!IMPORTANT]
> 历经多轮磨合与修正，这是自 2.0 以来我们最满意的里程碑版本。建议所有用户立即升级。

### 🐞 修复问题

- 修复首次启动时代理信息刷新缓慢
- 修复无网络时无限请求 IP 归属查询
- 修复 WebDAV 页面重试逻辑
- 修复 Linux 通过 GUI 安装服务模式权限不符合预期
- 修复 macOS 因网口顺序导致无法正确设置代理
- 修复恢复休眠后无法操作托盘
- 修复首页当前节点图标语义显示不一致
- 修复使用 URL scheme 导入订阅时没有及时重载配置
- 修复规则界面里的行号展示逻辑
- 修复 Windows 托盘打开日志失败
- 修复 KDE 首次启动报错

<details>
<summary><strong> ✨ 新增功能 </strong></summary>

- 升级 Mihomo 内核到最新
- 支持订阅设置自动延时监测间隔
- 新增流量隧道管理界面，支持可视化添加/删除隧道配置
- Masque 协议的 GUI 支持

</details>

<details>
<summary><strong> 🚀 优化改进 </strong></summary>

- 安装服务失败时报告更详细的错误
- 避免脏订阅地址无法 Scheme 导入订阅
- macOS TUN 覆盖 DNS 时使用 114.114.114.114
- 连通性测试替换为更快的 http://1.0.0.1
- 连接、规则、日志等页面的过滤搜索组件新增了清空输入框按钮
- 链式代理增加明显的入口出口与数据流向标识
- 优化 IP 信息卡
- 美化代理组图标样式
- 移除 Linux resources 文件夹下多余的服务二进制文件

</details>
