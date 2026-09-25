# Parano1d ①

> **v2 激活：主网区块 210537。** 本文档描述 v2，此高度之前适用 v1.1。
> 计划估计时间为 2026 年 10 月 10 日 23:59 PDT（10 月 11 日 06:59 UTC）；
> 激活由高度决定，而非日期。旧规则与测量见[归档](archive/index.md)。

**由工作量证明排序的 Proof-native Layer 1。从活价值到活权利。**

Parano1d 无需重放历史交易即可验证当前 State。钱包证明授权，矿工证明公开执行
及精确 State 转换，节点验证递归 `HistoryStep` 后应用已证明的写入。
每个终端认证当前状态及其有效祖先链。

## 从活价值到活权利

付款证明花费价值的权利。v2 合约还承诺**如何行使权利**：谁能操作、在哪个
高度、金额多少，以及计数器如何变化。调用证明允许的后继转换或关闭权利，
该证明成为区块有效性的一部分。

节点保留活承诺，不要求永久合约执行日志。参与者保存公开条款与可携带回执。
预算、预付服务、委托支出、延迟访问和分期付款因此可以使用与普通转账相同的
递归验证模型。

有界整数内核提供 16 条指令、两个持久计数器、检查算术、条件及高度访问。
任何用户都能在此 ABI 内创建程序，无需专用电路或矩阵。GUI、CLI 和 RPC
提供六种模板和自定义程序。执行需要授权调用，共识不运行后台计时器。

[理解 proof-native 合约](concepts/proof-native-contracts.md) ·
[创建和使用](contracts/index.md) · [API](contracts/api.md) ·
[GUI 指南](contracts/gui.md)

## 节点保留什么

State 是活输出的精确稀疏向量。已花费槽位清空后通过新的创建标识安全复用，
空分段为虚拟状态。节点保留永久紧凑区块头、递归终端、最近 42 个区块体和
有界回滚数据。新节点验证 State 和近期后缀，不重放旧执行。

区块头验证仍随链高度增长，State 传输随活集合增长。第三方可以归档公开交易。
零知识保护花费秘密，裁剪不等于隐藏数据。

[架构](architecture/overview.md) · [同步](architecture/synchronization.md)

## v2 配置

| 参数 | 值 |
| --- | --- |
| 目标间隔 / ASERT 半衰期 | 30 s / 180 s |
| Small, m23 | 63 页 / 504 输入 / 63 调用 |
| Large, m24 | 206 页 / 504 输入 / 63 调用 |
| Large 生产 | 服务器选择启用 `--v2-large-blocks` |
| 硬最终性 / 最大重组 | 18 / 17 区块 |
| 终端上限 | 1,100,000 字节 |

预算同时适用，调用与普通付款共享页数。Large 可在共同输入限制内容纳
63 次调用加 143 笔单页付款。主 coinbase 单独计数，额外强制系统页占一个
有效页。所有节点都验证两类。

仅靠算力不能创建区块：生产者先证明转换，再搜索 nonce。
见[挖矿](mining/index.md)、[参数](protocol/parameters.md)及[实测结果](reference/performance.md)。

## 发行与 State 效率

准确计划为 **16 → 11.30 → 8 → 5.65 → 4 → 2.83 → 2 → 1.41 → 1 NOID**，
从 v2 起每 1,051,200 个区块变化一次，1 NOID 尾部永久持续。
占用率独立决定净新增活槽位价格，该费用销毁，合并可避免增长费。
[网络经济](protocol/economics.md)解释常量及为何不再用 State 作为发行时钟。

## 证明栈与安全

Poseidon2b 和二元域算术连接授权、精确 State 及递归祖先链。
[FROST-GKR](research/frost-gkr.md)批处理公开哈希工作，透明证明栈无需可信设置。
最终 v2 矩阵库及旧矩阵退役证明具有与源码关联的 Category 1 资源计算，依赖
明确的组合、密码学及预处理假设。准确声明与边界见[安全模型](protocol/security-model.md)，

## 开始

- [安装原生钱包](getting-started/wallet.md)。
- [创建合约或打开共享文件](contracts/gui.md)。
- [运行 Core](getting-started/core.md) 或[挖矿](mining/index.md)。
- [集成 JSON-RPC](reference/rpc.md)。
- [从源码构建](developers/build.md)，检查固定的证明材料。

源代码定义共识行为，文档解释这些规则及建立在其上的应用流程。
