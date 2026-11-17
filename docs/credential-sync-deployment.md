# 凭据同步服务部署记录

当前个人自用服务部署在 Tailscale 地址 `100.100.1.1:18443`，容器名为 `cockpit-credential-sync-personal`，数据目录为服务器上的 `~/services/cockpit-credential-sync-server/data`。该端口独立于服务器已有的 8080、8089 和其他服务。

健康检查：

```bash
curl --fail http://100.100.1.1:18443/health
```

正常返回协议版本和状态，不返回 Token、设备密钥或管理员密钥。

管理员密钥只保存在服务器的受限 `.env` 文件中；不写入 Git，也不放入客户端配置。客户端只使用设备密钥签名请求。服务器当前以 Docker 非 root 用户运行，容器设置了 `no-new-privileges` 和 `cap-drop ALL`。

已完成验证：

- Rust 服务单元和协议测试通过。
- 新数据库启动和 HTTP 健康检查通过。
- 签名请求、nonce 防重放、撤销设备、备份版本冲突、密文篡改拒绝通过。
- 服务端和客户端加密测试覆盖 AES-GCM、HKDF、Ed25519、X25519 密钥封装、稳定账号标识和密钥轮换。

尚未完成：

- 尚未让 Cockpit Tools 客户端接入该地址。
- 尚未进行真实 OAuth 凭据上传或恢复。
- 尚未配置公网 HTTPS；服务只应通过 Tailscale 网络访问。
- 管理员密钥轮换、数据库备份恢复和设备配对的真实多设备验收仍需完成。
