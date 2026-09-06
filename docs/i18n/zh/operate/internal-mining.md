# 内置挖矿

内置挖矿把交易选择、证明构建、nonce 搜索和区块提交全部保留在同一个 Core
进程中。

## 准备

先运行发布版硬件检查：

```sh
parano1d --check-hardware
```

从节点钱包获取奖励地址：

```sh
parano1d
parano1d-cli address --list
parano1d-cli stop
```

未显式配置奖励地址时，会自动使用钱包活动地址。

## 启动

在前台运行：

```sh
parano1d --mode miner --cpu-threads 12
```

也可更新 systemd unit：

```ini
ExecStart=/usr/local/bin/parano1d \
  --config /etc/parano1d/parano1d.toml \
  --mode miner \
  --cpu-threads 12
```

然后重新加载并重启：

```sh
sudo systemctl daemon-reload
sudo systemctl restart parano1d
```

## 就绪条件

普通挖矿需要一个经过认证的对等节点，以及已同步的链。检查：

```sh
parano1d-cli status
parano1d-cli peers
parano1d-cli mining
```

进程会准备内嵌的 B25 与 B255 证明矩阵，并选择最合适的 CPU 后端。每个
挖矿会话都从 B25 开始。第一次完成的准备满足 `prepare_time_B25 × 4 ≤ 20 秒` 时允许 B255，随后由所选合格页面数量决定模板类别。详见[挖矿架构](../architecture/mining.md)。

## CPU 规划

`--cpu-threads` 是证明阶段和 PoW 阶段共用的总预算，不应超过 cgroup 或
虚拟机实际分配给服务的逻辑 CPU 数。

基础设施节点应为操作系统和公网 P2P 服务留出资源；专用矿机可以使用全部
可见逻辑 CPU。

钱包交易证明在本机优先于正在进行的挖矿，但这不会改变其他节点接受哪些
交易或区块。

## 更改奖励地址

每个新模板都会重新解析钱包活动地址。地址变化会在安全边界使本地工作失效
或刷新；已经不可变的模板不能改写奖励地址。

若要固定独立的进程奖励地址：

```sh
parano1d --mode miner --miner-address o1...
```

请使用完整 bech32m 地址。

## 停止

通过 RPC 或服务管理器停止：

```sh
parano1d-cli stop
```

```sh
sudo systemctl stop parano1d
```

正常关闭会取消挖矿、关闭网络并刷入 MDBX。不要仅因活动证明需要几秒到达
安全取消边界，就反复发送强制终止信号。
