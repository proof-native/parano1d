# 在 Linux 上运行节点

普通 Parano1d 节点验证完整区块、维护 [Live State](../reference/glossary.md#live-state)、中继交易并提供
同步数据，但不参与挖矿。

本指南在带有 systemd 的 64 位 Linux 服务器上，把发布版 Core 安装为
系统服务。

## 要求

发布版证明后端要求：

- x86-64，支持 SSE4.1 和 PCLMULQDQ；或
- ARM64，支持 NEON 和 PMULL。

运行时会自动选择 `pclmul`、`avx2+vpclmul`、`avx512bw+vpclmul`
或 `neon+pmull` 后端。发布版节点不使用标量参考后端。

节点通过 TCP `9600` 接受 P2P 连接。JSON-RPC 应保持绑定
`127.0.0.1:9601`。

主要可变存储随 Live State 中的 UTXO 数量变化。每个 212 字节的紧凑区块头会
永久保存，而完整区块体只保留最近 42 个。

购买虚拟机或设置磁盘、内存限制前，请阅读
[硬件与容量](hardware.md)。

## 安装 Core

从[发布页面](https://github.com/proof-native/parano1d/releases)下载与服务器
架构匹配的压缩包和 `SHA256SUMS`。解压前先校验，把 `VERSION` 换成实际
版本号：

```sh
grep '  parano1d-core-vVERSION-linux-x86_64.tar.gz$' SHA256SUMS \
  | sha256sum --check
```

命令必须报告 `OK`。ARM64 请把 `linux-x86_64` 替换为
`linux-aarch64`。

解压并运行硬件检查：

```sh
tar -xzf parano1d-core-vVERSION-linux-x86_64.tar.gz
./parano1d --check-hardware
```

支持的机器会以以下内容结束：

```text
NODE READY
```

安装节点和 CLI：

```sh
sudo install -m 0755 parano1d parano1d-cli /usr/local/bin/
```

## 创建服务账户

节点数据应与交互式用户账户分离：

```sh
sudo useradd --system --home-dir /var/lib/parano1d \
  --create-home --shell /usr/sbin/nologin parano1d
sudo install -d -o parano1d -g parano1d -m 0700 /var/lib/parano1d
sudo install -d -o root -g parano1d -m 0750 /etc/parano1d
```

创建 `/etc/parano1d/parano1d.toml`：

```toml
[network]
listen = "0.0.0.0:9600"
seeds = []

[storage]
backend = "mdbx"
path = "/var/lib/parano1d"

[rpc]
listen = "127.0.0.1:9601"

[mining]
enabled = false
miner_address = ""
```

保护配置：

```sh
sudo chown root:parano1d /etc/parano1d/parano1d.toml
sudo chmod 0640 /etc/parano1d/parano1d.toml
```

无需填写种子地址。发布版二进制会通过内置 [DNS 种子](../reference/glossary.md#dns-seed)发现公网，并记住成功
连接过的出站节点。

## 通过 systemd 运行

创建 `/etc/systemd/system/parano1d.service`：

```ini
[Unit]
Description=Parano1d node
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
User=parano1d
Group=parano1d
ExecStart=/usr/local/bin/parano1d --config /etc/parano1d/parano1d.toml
Restart=on-failure
RestartSec=5
KillSignal=SIGINT
TimeoutStopSec=45
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

`KillSignal=SIGINT` 给节点时间关闭网络服务并正常刷入 MDBX。

加载 unit 并启动节点：

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now parano1d
sudo systemctl status parano1d
```

跟踪启动与同步：

```sh
sudo journalctl -u parano1d -f
```

## 检查节点

CLI 默认连接本地 RPC：

```sh
parano1d-cli status
parano1d-cli peers
parano1d-cli state
```

`status` 应报告当前高度，`peers` 应变为非零，`state` 则显示经过认证的
Live State 的大小。

## 网络访问

在主机和服务商防火墙中允许入站 TCP `9600`。若服务器位于 NAT 后，将
TCP `9600` 转发给节点。节点只靠出站连接也能同步，但接受入站连接才能
更好地为网络服务。

不要公开 TCP `9601`。远程管理应使用 SSH 隧道或其他经过认证的私有
传输。

## 停止或更新

替换二进制或复制数据前先停止服务：

```sh
sudo systemctl stop parano1d
```

安装已校验的新二进制，再启动：

```sh
sudo install -m 0755 parano1d parano1d-cli /usr/local/bin/
sudo systemctl start parano1d
parano1d-cli status
```

普通软件更新不应删除 `/var/lib/parano1d`。若节点钱包收到资金，请单独
备份 `/var/lib/parano1d/wallet.key`，并把它作为私密密钥保护。
