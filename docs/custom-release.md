# 个人自用安装包

每次合并到 `custom` 后，`Custom desktop packages` 针对准确提交生成 macOS arm64 / x64 / universal、Windows x64、Linux x64 / arm64 安装包。新任务不会取消前一次合并的打包。

## 签名策略

无需 Apple Developer ID 证书和公证；macOS 使用临时签名，不标称 Apple 签名或公证。Windows 不依赖代码签名证书。所有更新文件独立使用 Tauri 更新签名；最终汇总验证每份文件签名、公钥、来源提交和版本，缺失任一平台时不创建发布。

`TAURI_SIGNING_PRIVATE_KEY` 和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 存在仓库 Actions Secrets；私钥不能提交到 Git。当前本机备份在 `~/.config/cockpit-tools-release/`，权限限制为当前用户。应自行保管离线副本，丢失私钥会使已安装客户端无法信任新签名。

## 版本与发布

版本为 `2.0.N`，N 是受保护 `custom` 提交的 first-parent 提交数。不能强制改写 `custom` 历史。重跑相同 SHA 保留相同版本；仅在隔离构建工作区修改版本文件，不自动提交版本更新，避免循环构建。

全部平台及来源验证成功后创建完整草稿 Release，包含更新清单、SHA256SUMS、按平台记录的来源元数据和安装包。失败时保留 Actions 诊断，不生成不完整 Release。首次发布先保留草稿，实机验收后再公开；更新器只读取已公开的 fork Release。首次从官方版改用定制版需手动安装。

fork 更新源为 `https://github.com/Mickls/cockpit-tools/releases/latest/download/latest.json` 及目标平台清单。官方原有 Release 工作流在这个 fork 上禁用，避免意外要求 Apple 凭据或使用官方发布策略。

实机验收分别检查：安装启动、账号及实例保留、升级后配对密钥保留、更新源持续指向 fork。CI 构建通过不代表这些真实设备验收已通过。
