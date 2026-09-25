# 合约速查

主网 v2 在 **H210537** 激活。本指南描述 v2，此前链遵循 v1.1。
公共合约 ABI 为 **3**：16 条指令、两个持久 u64 计数器、两个临时寄存器。
所有程序使用共享区块证明。Small m23 为 63 页 / 504 输入 / 63 调用，
服务器手动启用的 Large m24 为 206 / 504 / 63，调用与付款共享预算。

六种构造器为可退款付款、定时保险库、限额钱包、周期预算、定期付款及分期释放。
自定义程序使用相同检查整数内核和显式分支权限。条款承诺授权方、收款人、
期限、手续费及付款上限和保留额。各出资输出独立，计划转账需要调用。

**保存条款没有创建费。** 充值是单独普通付款。调用手续费来自合约余额，
受策略上限约束。继续创建后继，关闭转出余额且没有后继。截止高度起恢复分支生效。

GUI：**F7 Contracts**，选择 Create、My contracts 或 Open file。本地日志显示
双方保留交互。导入合并已验证证据，按交易 ID 去重并保留自己的记录。
F4 用于普通付款回执，合约文件和调用回执在 F7 打开。

CLI 从 `parano1d-cli contract protocol`、`parano1d-cli contract --help` 开始。
RPC 使用 `paranoid_` 命名空间和本地所有者权限。`getContractProtocol` 查询限制，
`createObject` 创建条款，`walletFundObject` 充值，`previewObjectCall` 预览，
`walletCallObject` 提交。选择准确 slot 和 creation ID，授权前绑定审阅过的交易体。
提交后仍需确认。

请保存钱包秘密、公开条款和回执。State 认证活余额，但不能从承诺恢复丢失程序。
回执在交易体裁剪后证明过去调用，当前可花费性需单独查询。共享合约文件携带
条款及最多一份匹配回执，需要更多历史时请分别交换其他回执。

完整[合约文档](../contracts/index.md)包括[内核](../contracts/core.md)、
[模板](../contracts/templates.md)、[API](../contracts/api.md)、
[GUI](../contracts/gui.md)和[恢复](../contracts/receipts-and-recovery.md)。
发布包读者可访问 [docs.parano1d.org](https://docs.parano1d.org/zh/contracts)。
