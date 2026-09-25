# 维护

Parano1d 以事务方式持久化 Live State。日常维护无需重放或永久保留历史
区块体。

## 监控

使用本地 CLI：

```sh
parano1d-cli status
parano1d-cli peers
parano1d-cli state
parano1d-cli mempool
parano1d-cli mining
```

systemd 环境：

```sh
systemctl status parano1d
journalctl -u parano1d --since today
```

应监控 MDBX 中的 Live State 和证明缓存实际使用的磁盘空间，而不是假设一个固定
容量。Live State 存储随当前 UTXO 使用量变化。

## 文件操作前先停止

复制数据库或替换二进制文件前，必须停止进程：

```sh
sudo systemctl stop parano1d
```

等待服务进入 inactive。不要用普通文件复制工具复制正在运行的 MDBX
目录并假设结果一定一致。

## 备份钱包权限

最关键的文件是：

```text
DATA_DIR/wallet.key
```

它包含未加密的 256 位主密钥。请复制到离线、受访问控制的位置，并保留
仅所有者可读的权限。

如需付款证据，还应备份：

```text
DATA_DIR/wallet.receipts
```

P2P 身份文件用于维持稳定的对等节点 ID，但没有花费权限。

## 更新

1. 下载新压缩包和对应的 `SHA256SUMS`；
2. 校验摘要；
3. 停止服务；
4. 同时替换 `parano1d`、`parano1d-cli` 和 `parano1d-miner`；
5. 启动服务；
6. 检查启动和同步状态。

普通更新不应删除数据目录。

## 重建链数据

如果怀疑链数据或 State 损坏，并且网络中有健康对等节点：

```sh
parano1d --purge-state
```

该命令会清除完整链数据库，包括区块头、保留的完整区块、链索引、撤销数据和
Live State，然后强制重新进行认证同步。钱包文件、收据和对等节点身份另行
存储，不会被删除；即便如此，操作前仍应备份钱包密钥和收据。这是恢复手段，
不是日常清理。

## 快照暂存

接收的快照属于临时数据。传输中断后，下次启动会先丢弃旧临时区，再开始
新的暂存会话。不要手动把 `snapshot-staging` 中的文件提升到规范数据库。

## 数据库与最终性

State 撤销数据保留 36 个区块，完整区块体保留 42 个。这些窗口由程序自动
管理。增加本地磁盘保留量不会改变共识中的 18 区块最终性规则。

## 合约数据

请与钱包秘密一起备份 `wallet.contracts.json`、`contract-activity/` 及完整
`objects/`（包含 `objects/terminals/`）。公开条款、活动及证明证据不能仅从
秘密恢复。停止节点后执行一致备份，见[合约恢复](../contracts/receipts-and-recovery.md)。
