# Upstream Main 下次合并流程

详细说明见: [upstream-main-merge.zh-CN.md](/D:/Me/clash-verge-rev/docs/upstream-main-merge.zh-CN.md)

这份文档是给下次直接照着执行用的，不解释原理，只保留实际步骤。

## 固定规则

- 只用 `merge`
- 不用 `rebase`
- 先合到同步分支，再合回 `personal`
- 冲突优先保留本仓库明确存在的产品差异

## 合并前检查

先确认当前工作区是干净的：

```powershell
git status
```

如果不是干净状态，先提交或处理完手头改动，再开始同步 upstream。

## 1. 切到 `personal` 并更新远端

```powershell
git switch personal
git fetch origin --prune
git fetch upstream --prune
git pull --ff-only origin personal
```

如果本机还没配过 `upstream`：

```powershell
git remote add upstream https://github.com/clash-verge-rev/clash-verge-rev.git
git config rerere.enabled true
git config merge.conflictstyle zdiff3
git fetch upstream --prune
```

## 2. 新建本次同步分支

按日期建分支：

```powershell
git switch -c sync/upstream-main-YYYYMMDD
```

示例：

```powershell
git switch -c sync/upstream-main-20260410
```

## 3. 发起 merge

```powershell
git merge --no-ff upstream/main
```

## 4. 解决冲突时按这个规则处理

- 不恢复 `home` 页面
- 保留 `personal` 的版本号
- 保留 Mihomo 的本地 path 依赖
- 代理页、根路由、已有产品行为优先保留本仓库实现
- 生成文件不要手工改，直接重新生成
- lockfile 不要手工拼，直接重新生成

重点看这些文件：

- `package.json`
- `pnpm-lock.yaml`
- `src-tauri/Cargo.toml`
- `Cargo.lock`
- `src/pages/_routers.tsx`
- `src/pages/profiles.tsx`
- `src/pages/proxies.tsx`
- `src-tauri/src/cmd/profile.rs`

## 5. 冲突解决后执行这些命令

```powershell
pnpm i18n:types
cargo generate-lockfile
pnpm install --lockfile-only --ignore-scripts --no-frozen-lockfile
pnpm install --ignore-scripts --no-frozen-lockfile
```

如果有新依赖缺失，再补装一次，比如之前实际补过：

```powershell
pnpm add validator@^13.15.26 --save-prod --lockfile-only
pnpm add @types/validator@^13.15.10 --save-dev --lockfile-only
pnpm add eslint-config-prettier --save-dev --lockfile-only
pnpm add eslint-plugin-prettier --save-dev --lockfile-only
```

如果改了前端格式，执行：

```powershell
pnpm exec prettier --write src eslint.config.ts package.json
```

如果改了 Rust 文件，至少对改过的文件执行：

```powershell
rustfmt src-tauri/src/cmd/profile.rs
```

## 6. 验证

优先跑：

```powershell
pnpm lint
pnpm typecheck
cargo test --workspace
```

如果 workspace 测试仍然卡在 upstream 自己的 `tauri-plugin-mihomo` 测试，就补跑：

```powershell
cargo check -p clash-verge
cargo test -p clash-verge --lib
```

目前已知可能失败的报错特征：

```text
Cannot start a runtime from within a runtime
```

## 7. 提交同步分支

```powershell
git status
git diff --check
git add -A
git commit -m "merge: sync upstream/main YYYY-MM-DD"
```

## 8. 合回 `personal`

```powershell
git switch personal
git merge --no-ff sync/upstream-main-YYYYMMDD -m "merge: integrate sync/upstream-main-YYYYMMDD"
```

## 9. 推送

```powershell
git push origin personal
```

## 10. 下次继续合并前先看这两个文档

- [upstream-main-merge-checklist.zh-CN.md](/D:/Me/clash-verge-rev/docs/upstream-main-merge-checklist.zh-CN.md)
- [upstream-main-merge.zh-CN.md](/D:/Me/clash-verge-rev/docs/upstream-main-merge.zh-CN.md)
