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

---

## v(2.4.6)

### 🐞 修复问题

- 修复首次启动时代理信息刷新缓慢
- 修复无网络时无限请求 IP 归属查询
- 修复 WebDAV 页面重试逻辑
- 修复 Linux 通过 GUI 安装服务模式权限不符合预期
- 修复 macOS 因网口顺序导致无法正确设置代理
- 修复恢复休眠后无法操作托盘

<details>
<summary><strong> ✨ 新增功能 </strong></summary>

- 支持订阅设置自动延时监测间隔
- 新增流量隧道管理界面，支持可视化添加/删除隧道配置

</details>

<details>
<summary><strong> 🚀 优化改进 </strong></summary>

- 安装服务失败时报告更详细的错误
- 避免脏订阅地址无法 Scheme 导入订阅

</details>
