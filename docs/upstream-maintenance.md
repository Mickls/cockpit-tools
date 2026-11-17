# 上游同步与分支维护

本 fork 只保留两个长期远端分支：

- `main`：官方仓库正式版本的镜像。
- `custom`：定制功能和最终发布版本，作为默认分支。

本地开发可以使用 `codex/*` 分支。推送后的临时分支用于 PR，合并到 `custom` 后由仓库设置自动删除。

## 自动同步

`.github/workflows/upstream-sync.yml` 每天 03:00 UTC（上海时间 11:00）运行，也可以手动触发。它只检查官方最新的非草稿、非预发布 Release：

1. 官方版本已包含在 `main` 且 `custom` 已包含选定镜像提交：工作流结束，不创建分支或 PR。若仍有待集成更新，则继续处理已有 PR。
2. `main` 落后：`main` 快进到官方 Release 对应的提交。
3. `custom` 尚未包含该提交：创建 `codex/upstream-<commit>` 分支和指向 `custom` 的 PR。
4. 已有相同上游提交的 PR：继续检查并尝试自动合并，不重复创建。
5. 检查失败或存在冲突：保留 PR，人工处理；不会生成发布包。

同步脚本位于 `scripts/release/upstream-sync.cjs`，测试位于 `scripts/release/upstream-sync.test.cjs`。运行：

```bash
node --test scripts/release/upstream-sync.test.cjs
```

脚本遇到官方历史分叉时拒绝强制推送。合并使用 merge commit，以保留上游祖先关系。仓库的 `delete_branch_on_merge` 设置负责删除已合并源分支；本地工作区的临时分支由开发者确认未包含未提交工作后清理。

## 权限与检查

`custom` 分支要求名为 `Custom integration checks` 和 `Custom regression checks` 的必需检查，并禁止强制推送和删除。前者汇总全部平台矩阵与预检查；后者包含发布脚本测试和前端类型检查。同步工作流需要能够写入 `main`、创建 PR 和请求自动合并；使用专用 GitHub App 或 `SYNC_TOKEN` 才能让机器人创建的 PR 触发后续检查。缺少该密钥时，默认 `GITHUB_TOKEN` 只能创建待处理 PR，不应绕过必需检查；管理员需手动触发后续验证。

不要把 Token、签名私钥或其他凭据写入工作流、脚本、PR 正文或日志。同步服务和客户端发布使用独立的密钥配置。
