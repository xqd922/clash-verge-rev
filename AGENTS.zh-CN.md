# Repository Guidelines

## 项目结构与模块划分
`src/` 是 React 19 + Vite 前端主目录。界面组件放在 `components/`，页面放在 `pages/`，公共逻辑放在 `hooks/`、`providers/`、`services/`，通用工具放在 `utils/`。多语言资源位于 `src/locales/`，生成的 TypeScript 类型位于 `src/types/generated/`。

`src-tauri/` 是桌面端壳层和主 Rust 应用，资源、图标与 sidecar 也在这里。可复用 Rust crate 放在 `crates/`，例如 `clash-verge-i18n`、`clash-verge-logging`、`tauri-plugin-mihomo`。构建与发布脚本在 `scripts/`，流程辅助脚本在 `scripts-workflow/`，文档在 `docs/`。

## 构建、测试与开发命令
- `pnpm dev`：启动默认 Tauri 开发环境。
- `pnpm dev:tauri`：以 Tauri 开发配置运行。
- `pnpm web:dev`：只启动前端 Vite 开发服务。
- `pnpm run prebuild`：下载或刷新 Mihomo sidecar 二进制。
- `pnpm build` / `pnpm build:fast`：构建正式包或快速测试包。
- `pnpm lint`：运行前端 ESLint。
- `pnpm typecheck`：执行 TypeScript 类型检查。
- `cargo test --workspace`：运行整个 Rust workspace 测试。
- `cargo test -p tauri-plugin-mihomo`：运行插件测试。
- `cargo make pre-commit` / `cargo make pre-push`：执行 Husky 对应检查。

## 代码风格与命名约定
遵循 `.editorconfig`：TS/JS/JSON/Markdown 使用 2 空格缩进，Rust 使用 4 空格，统一 UTF-8 与 LF。Prettier 负责前端格式化，规则包括 80 列、分号、双引号和尾随逗号；Rust 使用 `rustfmt`，宽度上限 120。

前端文件名优先使用 `kebab-case`，如 `traffic-sampler.ts`；React 组件使用 `PascalCase`；Rust crate 延续现有 `clash-verge-*` 命名。提交前运行 `pnpm format` 和 `cargo fmt`。

## 测试规范
前端改动至少应通过 `pnpm lint` 和 `pnpm typecheck`。Rust 改动应通过 `cargo clippy --all-targets --all-features -- -D warnings` 与相关 `cargo test`。Rust 测试放在各 crate 的 `tests/` 目录，文件名采用 `*_test.rs`。仓库当前未配置独立的前端单测框架，涉及 UI 行为修改时请在 `pnpm dev` 下手动验证，并在 PR 中附截图或录屏。

## 提交与 Pull Request 规范
近期提交消息使用简短前缀，如 `fix:`、`perf:`、`release:`。标题应使用祈使句，聚焦单一改动。仓库要求签名提交。

PR 需要说明改动内容、影响平台、关联 issue；如果涉及界面改动，附截图或录屏；如果涉及 sidecar、打包、迁移或配置变更，需要在描述中明确标注。
