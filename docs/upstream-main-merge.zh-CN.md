# Upstream Main 合并指南

英文版: [upstream-main-merge.md](/D:/Me/clash-verge-rev/docs/upstream-main-merge.md)

这个仓库通过 `upstream/main` 同步原项目，并把本地长期维护分支固定在
`personal`。

本仓库的固定策略是：

- 始终使用 `merge`
- 不使用 `rebase`
- 每次都先在专门的同步分支上处理 upstream 合并

## 为什么这里固定使用 `merge`

`personal` 里包含长期存在的产品差异、打包差异、本地插件路径依赖和版本决
策。如果改用 `rebase`，这些集成点会被改写，后续每次同步 upstream 都会更难
审计。使用 `merge` 可以保留真实集成历史，也能让 `git rerere` 复用冲突解法。

## 一次性初始化

在负责同步 upstream 的机器上，首次执行：

```powershell
git remote add upstream https://github.com/clash-verge-rev/clash-verge-rev.git
git config rerere.enabled true
git config merge.conflictstyle zdiff3
```

如果已经有 `upstream` 远端，就不要重复添加，只刷新即可：

```powershell
git fetch upstream --prune
```

## 标准合并 SOP

### 1. 从 `personal` 开始

```powershell
git switch personal
git fetch origin --prune
git fetch upstream --prune
git pull --ff-only origin personal
git switch -c sync/upstream-main-YYYYMMDD
```

示例：

```powershell
git switch -c sync/upstream-main-20260409
```

### 2. 把 upstream 合进同步分支

```powershell
git merge --no-ff upstream/main
```

注意：

- 不要直接在 `personal` 上合并 `upstream/main`
- 不要把 `personal` rebase 到 `upstream/main`
- 所有冲突都先在同步分支上解决

### 3. 按仓库规则处理冲突

当 upstream 和 `personal` 有长期分叉时，默认保留下面这些本地决策，除非这次
就是要主动改产品行为：

- 保留 `personal` 的路由结构，不恢复已经移除的 `home` 页面
- 保留 `package.json` 和 `src-tauri/Cargo.toml` 里的 `personal` 版本号
- 保留 `tauri-plugin-mihomo` 和 `tauri-plugin-mihomo-api` 的本地 path 依赖
- 保留 `personal` 已明确存在的代理页、根路由等产品行为
- 如果合并后的 upstream 代码需要新依赖，就补进来
- 生成文件不要手工合并冲突标记，直接重新生成
- lockfile 不要手工拼，直接重新生成

### 4. 重新生成生成文件和 lockfile

冲突解决后，按这次合并涉及的内容执行重新生成。这个仓库常用的是：

```powershell
pnpm i18n:types
cargo generate-lockfile
pnpm install --lockfile-only --ignore-scripts --no-frozen-lockfile
pnpm install --ignore-scripts --no-frozen-lockfile
```

如果 upstream 新增了依赖，且已经被合并进来的代码引用到了，就先显式补依赖，
再刷新 lockfile 和安装。

### 5. 做合并验证

优先验证集：

```powershell
pnpm lint
pnpm typecheck
cargo test --workspace
```

如果 `cargo test --workspace` 失败点属于 upstream 自己的测试目标，而不是这次
合并冲突处理造成的应用错误，就补一组更聚焦的应用验证，确认主程序仍可编译
和运行单测：

```powershell
cargo check -p clash-verge
cargo test -p clash-verge --lib
```

这种缩窄验证只作为兜底方案使用，并且要在合并记录里写清楚失败的是哪些工作
区测试。

### 6. 提交前复查

提交前至少检查：

```powershell
git status
git diff --check
git diff --stat
```

如果某些文件一直是冲突高发点，要单独再看一遍。

### 7. 提交同步分支

```powershell
git add -A
git commit
```

建议提交信息格式：

```text
merge: sync upstream/main YYYY-MM-DD
```

### 8. 把已验证的同步分支合回 `personal`

```powershell
git switch personal
git merge --no-ff sync/upstream-main-YYYYMMDD
git push origin personal
```

## 2026-04-09 这次实际执行的命令

这一节记录 `sync/upstream-main-20260409` 这次同步时，实际执行过的命令和决策。

### 建分支和发起合并

```powershell
git remote add upstream https://github.com/clash-verge-rev/clash-verge-rev.git
git config rerere.enabled true
git config merge.conflictstyle zdiff3
git fetch upstream --prune
git switch personal
git switch -c sync/upstream-main-20260409
git merge --no-ff upstream/main
```

### 合并后的重新生成和依赖刷新

```powershell
pnpm i18n:types
cargo generate-lockfile
pnpm install --lockfile-only --ignore-scripts --no-frozen-lockfile
pnpm install --ignore-scripts --no-frozen-lockfile
```

因为合并后的 upstream 代码已经引用了新依赖，所以额外补了：

```powershell
pnpm add validator@^13.15.26 --save-prod --lockfile-only
pnpm add @types/validator@^13.15.10 --save-dev --lockfile-only
pnpm add eslint-config-prettier --save-dev --lockfile-only
pnpm add eslint-plugin-prettier --save-dev --lockfile-only
```

冲突收尾时实际执行过的格式化命令：

```powershell
pnpm exec prettier --write src eslint.config.ts package.json docs/upstream-main-merge.md
rustfmt src-tauri/src/cmd/profile.rs
```

### 2026-04-09 这次保留的合并决策

- 保留 `personal` 现有产品结构，不恢复 `src/pages/home.tsx`
- 保留 `package.json` 里的 `7.0.14`
- 保留 `src-tauri/Cargo.toml` 里的 `7.0.14`
- 保留 `package.json` 中的本地 Mihomo path 依赖
- 保留 `src-tauri/Cargo.toml` 中的本地 Mihomo path 依赖
- 合并后把 `src-tauri/src/cmd/profile.rs` 调整到当前 upstream Rust API
- i18n 生成文件通过重新生成处理，不手工合并
- Cargo 和 pnpm 的 lockfile 通过重新生成处理，不手工拼冲突

### 2026-04-09 这次的验证结果

实际跑过的命令：

```powershell
pnpm lint
pnpm typecheck
cargo check -p clash-verge
cargo test -p clash-verge --lib
cargo test --workspace
```

结果：

- `pnpm lint`：通过
- `pnpm typecheck`：通过
- `cargo check -p clash-verge`：通过
- `cargo test -p clash-verge --lib`：通过
- `cargo test --workspace`：未全部通过，失败点在 `tauri-plugin-mihomo` 测试

2026-04-09 观察到失败的 workspace 测试：

- `restart`
- `reload_config`
- `mihomo_common_flush_dns`
- `mihomo_common_flush_fakeip`
- `mihomo_common_get_version`
- `mihomo_common_patch_base_config`
- `mihomo_common_update_geo`

报错特征是：

```text
Cannot start a runtime from within a runtime
```

这个失败来自 `tauri-plugin-mihomo` 的测试目标，不是 `src-tauri` 应用合并本身
产生的新编译错误。

## 以后每次合并都要记住的事

- 后续每次都保持 merge-only，不要来回切换 merge 和 rebase
- 让 `rerere` 持续学习重复冲突的解法
- 尽量把本地定制集中在有限文件里，减少未来冲突面
- 如果未来 upstream 的功能正式替代了某个本地决策，合并时要同步更新这份文档
