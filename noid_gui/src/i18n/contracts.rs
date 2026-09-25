// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

use crate::model::Language;

pub(super) fn translate(language: Language, source: &str) -> Option<String> {
    let pair = match source {
        "CREATE" => ("СОЗДАТЬ", "创建"),
        "MY CONTRACTS" => ("МОИ КОНТРАКТЫ", "我的合约"),
        "OPEN FILE" => ("ОТКРЫТЬ ФАЙЛ", "打开文件"),
        "CHOOSE A CONTRACT" => ("ВЫБЕРИТЕ КОНТРАКТ", "选择合约"),
        "Select one from your list, create a new contract or open a file you received." => ("Выберите контракт из списка, создайте новый или откройте полученный файл.", "从列表中选择合约、创建新合约或打开收到的文件。"),
        "CREATE CONTRACT" => ("СОЗДАТЬ КОНТРАКТ", "创建合约"),
        "Your created and imported contracts will appear here." => ("Здесь будут ваши созданные и добавленные контракты.", "您创建和导入的合约将显示在此处。"),
        "CONTRACT" => ("КОНТРАКТ", "合约"),
        "REFRESH LIST" => ("ОБНОВИТЬ СПИСОК", "刷新列表"),
        "CONTRACT NAME" => ("ИМЯ КОНТРАКТА", "合约名称"),
        "INITIAL DEPOSIT (NOID)" => ("НАЧАЛЬНОЕ ПОПОЛНЕНИЕ (NOID)", "首次充值（NOID）"),
        "CREATE & FUND" => ("СОЗДАТЬ И ПОПОЛНИТЬ", "创建并充值"),
        "SAVE WITHOUT DEPOSIT" => ("СОХРАНИТЬ БЕЗ ПОПОЛНЕНИЯ", "保存但不充值"),
        "Example: reserve 10 NOID for a recipient. They collect before expiry; you can recover the remainder from the expiry block." => ("Пример: выделите получателю 10 NOID. Он может забрать их до срока, а вы — вернуть остаток начиная с блока истечения срока.", "示例：为收款人预留 10 NOID。对方可在到期前领取；从到期区块起，您可收回余额。"),
        "Example: lock savings until a chosen block. The owner can withdraw after that block." => ("Пример: заблокируйте сбережения до выбранного блока. Начиная с него владелец сможет вывести средства.", "示例：将储蓄锁定至指定区块。从该区块起，所有者即可提款。"),
        "Example: give a spending key access to a funded balance with a maximum payment per call and a protected reserve." => ("Пример: предоставьте расходному ключу доступ к средствам с лимитом платежа на вызов и неснижаемым резервом.", "示例：授予支出密钥使用余额的权限，同时设置单次付款上限及最低储备。"),
        "Example: allow up to 10 NOID of payments and fees per period. Unused budget does not carry over." => ("Пример: разрешите тратить до 10 NOID на платежи и комиссии за период. Неиспользованный бюджет не переносится.", "示例：每周期允许支付和手续费合计最多 10 NOID。未使用的预算不会结转。"),
        "Example: prepay a recurring allowance. The recipient claims each due payment; nothing is charged automatically." => ("Пример: заранее пополните регулярное пособие. Получатель сам запрашивает очередной платёж; автоматических списаний нет.", "示例：预存定期津贴。收款人自行领取每笔到期付款，不会自动扣款。"),
        "Example: unlock 5 NOID at each interval. The beneficiary submits a call to claim each tranche." => ("Пример: разблокируйте по 5 NOID через заданные интервалы. Для получения каждой части получатель отправляет вызов.", "示例：每隔指定周期解锁 5 NOID。受益人提交调用以领取每笔分期款。"),
        "Creating and funding requires your confirmation. Later actions depend on the contract rules and the active wallet address." => ("Создание с пополнением требует вашего подтверждения. Дальнейшие действия зависят от правил контракта и активного адреса кошелька.", "创建并充值需要您确认。后续操作取决于合约规则及钱包当前地址。"),
        "Contract schedules use block heights. Each deposit has its own balance and counters." => ("Сроки контракта задаются высотой блока. У каждого пополнения свой остаток и счётчики.", "合约时间表使用区块高度。每次充值都有独立余额和计数器。"),
        "DEPOSIT AMOUNT (NOID)" => ("СУММА ПОПОЛНЕНИЯ (NOID)", "充值金额（NOID）"),
        "SPENDING AUTHORITY BEFORE DEADLINE" => ("КЛЮЧ РАСХОДОВАНИЯ ДО СРОКА", "到期前的支出授权方"),
        "SPENDING AUTHORITY FROM DEADLINE" => ("КЛЮЧ РАСХОДОВАНИЯ С НАСТУПЛЕНИЕМ СРОКА", "到期后的支出授权方"),
        "SHARE CONTRACT" => ("ПЕРЕДАТЬ КОНТРАКТ", "分享合约"),
        "OPERATIONS & RECEIPTS" => ("ОПЕРАЦИИ И ЧЕКИ", "操作与凭证"),
        "RULES" => ("ПРАВИЛА", "规则"),
        "YOUR ADDRESS CAN USE THIS CONTRACT" => ("ВАШ АДРЕС МОЖЕТ ИСПОЛЬЗОВАТЬ КОНТРАКТ", "您的地址可以使用此合约"),
        "YOUR ADDRESS HAS ACCESS AFTER THE DEADLINE" => ("ВАШ АДРЕС ПОЛУЧИТ ДОСТУП С УКАЗАННОГО БЛОКА", "您的地址在到期区块起获得访问权限"),
        "VIEW ONLY FOR THE ACTIVE ADDRESS" => ("АКТИВНЫЙ АДРЕС МОЖЕТ ТОЛЬКО ПРОСМАТРИВАТЬ", "当前地址仅可查看"),
        "ACCESS FROM BLOCK" => ("ДОСТУП С БЛОКА", "开放权限的起始区块"),
        "CONTRACT BALANCES" => ("ОСТАТКИ КОНТРАКТА", "合约余额"),
        "No available balance at this contract state. You can add funds or check another saved state below." => ("В этом состоянии доступных средств нет. Можно пополнить контракт или выбрать другое сохранённое состояние ниже.", "此状态没有可用余额。您可充值或查看下方其他已保存状态。"),
        "Each deposit has its own balance and counters. Select the one you want to use." => ("У каждого пополнения свой остаток и счётчики. Выберите нужное.", "每次充值都有独立余额和计数器。请选择要使用的一笔。"),
        "MORE DEPOSITS" => ("ДРУГИЕ ПОПОЛНЕНИЯ", "更多充值"),
        "Checking balances / awaiting confirmation…" => ("Проверяем остатки / ожидаем подтверждения…", "正在检查余额 / 等待确认…"),
        "MORE SAVED STATES" => ("ДРУГИЕ СОСТОЯНИЯ", "更多已保存状态"),
        "CHECK UPDATED BALANCE" => ("ПРОВЕРИТЬ НОВЫЙ ОСТАТОК", "检查更新后的余额"),
        "WITHDRAW / RETURN" => ("ВЫВЕСТИ / ВЕРНУТЬ", "提取 / 退回"),
        "COLLECT BALANCE" => ("ЗАБРАТЬ СРЕДСТВА", "领取余额"),
        "ADD FUNDS" => ("ПОПОЛНИТЬ", "充值"),
        "MAKE PAYMENT" => ("ПЕРЕВЕСТИ", "付款"),
        "CALL WITHOUT PAYMENT" => ("ВЫЗОВ БЕЗ ПЛАТЕЖА", "无付款调用"),
        "CHOOSE AN ACTION" => ("ВЫБЕРИТЕ ДЕЙСТВИЕ", "选择操作"),
        "REVIEW DEPOSIT" => ("ПРОВЕРИТЬ ПОПОЛНЕНИЕ", "核对充值"),
        "PAYMENT AMOUNT (NOID)" => ("СУММА ПЛАТЕЖА (NOID)", "付款金额（NOID）"),
        "The selected deposit is closed. Its remaining balance, less the fee, goes to this recipient." => ("Выбранный остаток будет закрыт. Все средства за вычетом комиссии поступят этому получателю.", "所选充值将被关闭，余额扣除手续费后发送给此收款人。"),
        "Run the program without making a payment. The network fee comes from the selected balance." => ("Выполнить программу без платежа. Комиссия сети вычитается из выбранного остатка.", "执行程序但不付款。网络手续费从所选余额中扣除。"),
        "REVIEW CONTRACT CALL" => ("ПРОВЕРИТЬ ВЫЗОВ", "核对合约调用"),
        "Select an available deposit first." => ("Сначала выберите доступный остаток.", "请先选择一笔可用充值。"),
        "The active address cannot perform this action at the next block." => ("Активный адрес не может выполнить это действие в следующем блоке.", "当前地址无权在下一个区块执行此操作。"),
        "The contract rules do not allow this action at the next block." => ("Правила контракта запрещают это действие в следующем блоке.", "合约规则不允许在下一个区块执行此操作。"),
        "CONTRACT ADDRESS" => ("АДРЕС КОНТРАКТА", "合约地址"),
        "Recent operations saved by this wallet. A receipt becomes available after confirmation." => ("Последние операции, сохранённые этим кошельком. Чек станет доступен после подтверждения.", "此钱包保存的近期操作。确认后即可获取凭证。"),
        "No recorded operations for this contract yet." => ("Для этого контракта ещё нет сохранённых операций.", "此合约尚无保存的操作。"),
        "AMOUNT NOT INCLUDED IN RECEIPT" => ("СУММА НЕ УКАЗАНА В ЧЕКЕ", "凭证未包含金额"),
        "TRANSACTION ID" => ("ID ТРАНЗАКЦИИ", "交易 ID"),
        "NOT INCLUDED IN RECEIPT" => ("НЕ УКАЗАНА В ЧЕКЕ", "凭证未包含"),
        "SAVE RECEIPT" => ("СОХРАНИТЬ ЧЕК", "保存凭证"),
        "Confirmed. The receipt proof is still being prepared; it will be checked again at the next block." => ("Подтверждено. Доказательство для чека ещё готовится; повторная проверка — на следующем блоке.", "已确认。凭证证明仍在准备中，将在下一个区块再次检查。"),
        "OPEN A CONTRACT YOU RECEIVED" => ("ОТКРОЙТЕ ПОЛУЧЕННЫЙ КОНТРАКТ", "打开收到的合约"),
        "Choose a contract file or a contract receipt. Review the rules and your access before adding it to your wallet." => ("Выберите файл контракта или его чек. Перед добавлением в кошелёк проверьте правила и свои права.", "选择合约文件或合约凭证。在添加到钱包之前，请核对规则及您的权限。"),
        "FILE PATH" => ("ПУТЬ К ФАЙЛУ", "文件路径"),
        "BROWSE…" => ("ВЫБРАТЬ…", "浏览…"),
        "FILE CHECKED · RECEIPT VERIFIED" => ("ФАЙЛ ПРОВЕРЕН · ЧЕК ПОДТВЕРЖДЁН", "文件已检查 · 凭证已验证"),
        "CONTRACT RULES LOADED" => ("ПРАВИЛА КОНТРАКТА ЗАГРУЖЕНЫ", "已加载合约规则"),
        "FILE" => ("ФАЙЛ", "文件"),
        "CALL AUTHORIZED BY" => ("КЛЮЧ АВТОРИЗАЦИИ ВЫЗОВА", "调用授权方"),
        "The receipt confirms a past operation. Available balances are checked separately." => ("Чек подтверждает прошлую операцию. Доступные остатки проверяются отдельно.", "凭证证明一笔历史操作。可用余额另行检查。"),
        "AVAILABLE IN THIS PAGE (NOID)" => ("ДОСТУПНО НА ЭТОЙ СТРАНИЦЕ (NOID)", "本页可用余额（NOID）"),
        "This state has no available balance. The file may describe a draft, a spent deposit or an older state." => ("В этом состоянии доступных средств нет. Файл может описывать контракт без пополнения, потраченный остаток или старое состояние.", "此状态没有可用余额。文件可能描述未充值的草稿、已花费的充值或旧状态。"),
        "ADD TO MY CONTRACTS & OPEN" => ("ДОБАВИТЬ В МОИ КОНТРАКТЫ И ОТКРЫТЬ", "添加到我的合约并打开"),
        "This receipt proves a closing call. That deposit was closed by the recorded operation." => ("Этот чек подтверждает закрытие. Указанная операция закрыла этот остаток контракта.", "此凭证证明一次关闭调用，所记录的操作已关闭该笔充值。"),
        "WAITING FOR A FILE" => ("ВЫБЕРИТЕ ФАЙЛ", "等待选择文件"),
        "Ask the sender to use Share contract in their wallet." => ("Попросите отправителя нажать «Передать контракт» в своём кошельке.", "请让发送方使用其钱包中的“分享合约”。"),
        "CREATED HERE" => ("СОЗДАН ЗДЕСЬ", "在此创建"),
        "FROM A FILE" => ("ИЗ ФАЙЛА", "来自文件"),
        "SAVED IN THIS WALLET" => ("СОХРАНЁН В КОШЕЛЬКЕ", "已保存在此钱包"),
        "DEPOSIT" => ("ПОПОЛНЕНИЕ", "充值"),
        "CONTRACT CALL" => ("ВЫЗОВ КОНТРАКТА", "合约调用"),
        "CHAIN CHANGED" => ("ЦЕПЬ ИЗМЕНИЛАСЬ", "链已改变"),
        "NOT CONFIRMED — REVIEW AGAIN" => ("НЕ ПОДТВЕРЖДЕНО — ПРОВЕРЬТЕ СНОВА", "未确认 — 请重新核对"),
        "AWAITING CONFIRMATION" => ("ОЖИДАЕТ ПОДТВЕРЖДЕНИЯ", "等待确认"),
        "Invalid funding quote." => ("Некорректный расчёт пополнения.", "充值报价无效。"),
        "Invalid operation ID." => ("Некорректный ID операции.", "操作 ID 无效。"),
        "Invalid block hash." => ("Некорректный хеш блока.", "区块哈希无效。"),
        "Invalid local contract activity." => ("Некорректный локальный журнал контракта.", "本地合约操作记录无效。"),
        "Contract activity exceeds its limit." => ("Журнал контракта превышает допустимый размер.", "合约操作记录超出限制。"),
        "Invalid contract operation." => ("Некорректная операция контракта.", "合约操作无效。"),
        "Invalid transaction height." => ("Некорректная высота транзакции.", "交易高度无效。"),
        "Invalid chain tip." => ("Некорректные данные вершины цепи.", "链顶数据无效。"),
        "A payment receipt needs the contract file to restore its rules." => ("Для восстановления правил к чеку платежа нужен файл контракта.", "付款凭证需要合约文件才能恢复规则。"),
        "Funding receipt is not confirmed on this chain." => ("Чек пополнения не подтверждён в этой цепи.", "充值凭证未在此链确认。"),
        "Funding receipt has no authenticated payment." => ("В чеке пополнения нет подтверждённых данных платежа.", "充值凭证没有经过验证的付款数据。"),
        "The receipt does not establish the contract state in this file." => ("Чек не подтверждает состояние контракта из этого файла.", "凭证未证明此文件中的合约状态。"),
        "The chain changed while checking this file. Open it again." => ("Во время проверки файла цепь изменилась. Откройте его снова.", "检查文件时链已改变，请重新打开。"),
        "Receipt block is no longer selected." => ("Блок чека больше не входит в выбранную цепь.", "凭证区块已不在选定链上。"),
        "No file selected. You can also paste its full path and choose Open file." => ("Файл не выбран. Можно вставить полный путь и нажать «Открыть файл».", "未选择文件。您也可以粘贴完整路径并选择“打开文件”。"),
        "Unsupported contract file version." => ("Версия файла контракта не поддерживается.", "不支持此合约文件版本。"),
        "This file has no contract rules. Ask the sender to use Share contract." => ("В файле нет правил контракта. Попросите отправителя нажать «Передать контракт».", "此文件没有合约规则。请让发送方使用“分享合约”。"),
        "Contract terms exceed their size limit." => ("Условия контракта превышают допустимый размер.", "合约条款超出大小限制。"),
        "This receipt records a closed balance." => ("Этот чек подтверждает уже закрытый остаток.", "此凭证记录已关闭的余额。"),
        "Contract added to My contracts. Balances were checked on this node." => ("Контракт добавлен в «Мои контракты». Остатки проверены вашим узлом.", "合约已添加到“我的合约”。余额已由此节点检查。"),
        "File saving cancelled." => ("Сохранение файла отменено.", "已取消保存文件。"),
        "Contract file saved with a verified operation receipt. Share it with the other participant." => ("Файл контракта сохранён вместе с проверенным чеком операции. Передайте его другому участнику.", "合约文件已保存，并包含已验证的操作凭证。可将其分享给另一方。"),
        "Contract rules saved. The recipient will check current balances when opening this file." => ("Правила контракта сохранены. При открытии файла получатель проверит текущие остатки.", "合约规则已保存。接收方打开文件时将检查当前余额。"),
        "Funding amount overflow." => ("Сумма пополнения превышает допустимое значение.", "充值金额溢出。"),
        "The receipt does not fund the contract in this file." => ("Этот чек не подтверждает пополнение контракта из файла.", "此凭证未证明向文件中的合约充值。"),
        "Missing contract storage directory." => ("Не задан каталог хранения контрактов.", "缺少合约存储目录。"),
        "Verified operation receipt saved." => ("Проверенный чек операции сохранён.", "已保存经过验证的操作凭证。"),
        "Saved without a deposit. Open My contracts to fund it later." => ("Сохранено без пополнения. Позже пополните через «Мои контракты».", "已保存但未充值。稍后可在“我的合约”中充值。"),
        "Choose a contract file first." => ("Сначала выберите файл контракта.", "请先选择合约文件。"),
        "Open and check a contract file first." => ("Сначала откройте и проверьте файл контракта.", "请先打开并检查合约文件。"),
        "Operation list changed. Refresh it." => ("Список операций изменился. Обновите его.", "操作列表已改变，请刷新。"),
        "Choose a contract first." => ("Сначала выберите контракт.", "请先选择合约。"),
        "CONTRACT RECEIPTS" => ("КВИТАНЦИИ КОНТРАКТОВ", "合约凭证"),
        "SPENDING RULES. ENFORCED BY THE BLOCK PROOF." => ("ПРАВИЛА РАСХОДОВАНИЯ. ПОДТВЕРЖДЕНЫ ДОКАЗАТЕЛЬСТВОМ БЛОКА.", "支出规则，由区块证明确保执行。"),
        "Choose a template or write a custom program. Create terms, then fund and use the contract." => ("Выберите шаблон или напишите программу. Создайте условия, затем пополните контракт и пользуйтесь им.", "选择模板或编写自定义程序。创建条款后，即可充值并使用合约。"),
        "CURRENT BLOCK" => ("ТЕКУЩИЙ БЛОК", "当前区块"),
        "NEW CONTRACT" => ("НОВЫЙ КОНТРАКТ", "新建合约"),
        "BACK TO CONTRACT" => ("ВЕРНУТЬСЯ К КОНТРАКТУ", "返回合约"),
        "NO SAVED CONTRACTS YET" => ("ПОКА НЕТ СОХРАНЁННЫХ КОНТРАКТОВ", "尚无保存的合约"),
        "Create or import terms to keep them here." => ("Создайте или импортируйте условия — они появятся здесь.", "创建或导入条款后，将显示在此处。"),
        "UNNAMED CONTRACT" => ("КОНТРАКТ БЕЗ ИМЕНИ", "未命名合约"),
        "CHOOSE A TEMPLATE" => ("ВЫБЕРИТЕ ШАБЛОН", "选择模板"),
        "Creating terms does not move funds. Review the rules before making a deposit." => ("Создание условий не перемещает средства. Проверьте правила перед пополнением.", "创建条款不会转移资金。充值前请核对规则。"),
        "DEADLINE BLOCK" => ("БЛОК СМЕНЫ ПРАВ", "权限切换区块"),
        "SPENDING AUTHORITY" => ("КЛЮЧ РАСХОДОВАНИЯ", "支出授权方"),
        "CLOSING RECIPIENT" => ("ПОЛУЧАТЕЛЬ ПРИ ЗАКРЫТИИ", "关闭时的收款人"),
        "SHOW PROGRAM" => ("ПОКАЗАТЬ ПРОГРАММУ", "显示程序"),
        "HIDE PROGRAM" => ("СКРЫТЬ ПРОГРАММУ", "收起程序"),
        "BALANCES & ACTIONS" => ("БАЛАНСЫ И ДЕЙСТВИЯ", "余额与操作"),
        "KEEP PROOF OF THE CALL." => ("СОХРАНИТЕ ДОКАЗАТЕЛЬСТВО ВЫЗОВА.", "保留调用证明。"),
        "A contract receipt proves a recorded call on the node's selected chain. Save it to share or verify later." => ("Квитанция подтверждает вызов в выбранной узлом цепи. Сохраните её, чтобы передать другому участнику или проверить позже.", "合约凭证可证明节点所选链上已记录的调用。保存后可分享或稍后验证。"),
        "VERIFY A RECEIPT" => ("ПРОВЕРКА КВИТАНЦИИ", "验证凭证"),
        "Open a contract .receipt file. The local node checks the proof and its inclusion in the chain." => ("Откройте файл контракта .receipt. Локальный узел проверит доказательство и включение вызова в цепь.", "打开合约 .receipt 文件。本地节点将验证证明及其链上记录。"),
        "SAVE A CALL RECEIPT" => ("СОХРАНЕНИЕ КВИТАНЦИИ ВЫЗОВА", "保存调用凭证"),
        "Select the call's contract terms in the first tab, then enter the confirmed transaction ID." => ("Выберите условия контракта для этого вызова в первом табе и укажите ID подтверждённой транзакции.", "在第一个标签页选择本次调用的合约条款，然后输入已确认的交易 ID。"),
        "SELECTED CONTRACT" => ("ВЫБРАННЫЙ КОНТРАКТ", "所选合约"),
        "No contract selected." => ("Контракт не выбран.", "尚未选择合约。"),
        "The receipt is verified before the file is saved." => ("Перед сохранением квитанция проходит проверку.", "保存文件前将先验证凭证。"),
        "RECEIPT VERIFIED" => ("КВИТАНЦИЯ ПРОВЕРЕНА", "凭证已验证"),
        "The recorded call closed this contract balance." => ("Записанный вызов закрыл этот остаток контракта.", "所记录的调用已关闭该笔合约余额。"),
        "The recorded call created a successor state." => ("Записанный вызов создал следующее состояние.", "所记录的调用创建了后续状态。"),
        "This proves the recorded call. Refresh current balances to check whether its successor remains spendable." => ("Подтверждён записанный вызов. Обновите текущие балансы, чтобы проверить, доступен ли остаток следующего состояния.", "此结果证明已记录的调用。请刷新当前余额，以确认后续状态是否仍可支出。"),
        "SUCCESSOR CONTRACT" => ("КОНТРАКТ ПОСЛЕ ВЫЗОВА", "后续合约"),
        "OPEN CONTRACT" => ("ОТКРЫТЬ КОНТРАКТ", "打开合约"),
        "NO RECEIPT CHECKED YET" => ("КВИТАНЦИЯ ЕЩЁ НЕ ПРОВЕРЕНА", "尚未验证凭证"),
        "The verification result will appear here." => ("Здесь появится результат проверки.", "验证结果将显示在此处。"),
        "OTHER SAVED COUNTERS AND BALANCES" => ("ДРУГИЕ СОХРАНЁННЫЕ СОСТОЯНИЯ И ОСТАТКИ", "其他已保存的状态和余额"),
        "Use a funded state after another participant calls the contract or the chain changes. All entries use the same program and spending rules." => ("Выберите состояние со средствами после вызова другим участником или изменения цепи. У всех записей одинаковые программа и правила расходования.", "其他参与者调用合约或链发生变化后，可选择仍有余额的状态。所有条目使用相同的程序和支出规则。"),
        "No other funded states on this page." => ("На этой странице нет других состояний со средствами.", "本页没有其他有余额的状态。"),
        "NEXT SAVED STATES" => ("СЛЕДУЮЩИЕ СОСТОЯНИЯ", "下一页已保存状态"),
        "No more saved states." => ("Больше сохранённых состояний нет.", "没有更多已保存状态。"),
        "Each deposit creates a separate balance with its own counters and limits. Deposits do not merge." => ("Каждое пополнение создаёт отдельный остаток со своими счётчиками и лимитами. Пополнения не объединяются.", "每次充值都会创建独立余额，并使用自己的计数器和限额。充值不会合并。"),
        "PERIOD BUDGET" => ("БЮДЖЕТ НА ПЕРИОД", "周期预算"),
        "RECURRING PAYMENT" => ("РЕГУЛЯРНЫЙ ПЛАТЁЖ", "定期付款"),
        "GRADUAL UNLOCK" => ("ПОСТЕПЕННАЯ РАЗБЛОКИРОВКА", "分期解锁"),
        "CUSTOM PROGRAM" => ("СВОЯ ПРОГРАММА", "自定义程序"),
        "RELOAD" => ("ОБНОВИТЬ", "重新加载"),
        "SAVED CONTRACTS" => ("СОХРАНЁННЫЕ КОНТРАКТЫ", "已保存的合约"),
        "REMOVE FROM LIST" => ("УБРАТЬ ИЗ СПИСКА", "从列表移除"),
        "LOCAL NAME" => ("ИМЯ В КОШЕЛЬКЕ", "本地名称"),
        "SAVE NAME" => ("СОХРАНИТЬ ИМЯ", "保存名称"),
        "ACTIVATION BLOCK" => ("БЛОК АКТИВАЦИИ", "激活区块"),
        "MATURITY BLOCK" => ("БЛОК ПОЛНОЙ РАЗБЛОКИРОВКИ", "完全解锁区块"),
        "FIRST PERIOD / UNLOCK BLOCK" => ("ПЕРВЫЙ БЛОК ПЕРИОДА / РАЗБЛОКИРОВКИ", "首个周期 / 解锁区块"),
        "PERIOD IN BLOCKS" => ("ПЕРИОД В БЛОКАХ", "周期区块数"),
        "FIXED PAYMENT / TRANCHE (NOID)" => ("ФИКСИРОВАННЫЙ ПЛАТЁЖ / ТРАНШ (NOID)", "固定付款 / 分期金额（NOID）"),
        "TOTAL PERIOD BUDGET (NOID)" => ("ОБЩИЙ БЮДЖЕТ ПЕРИОДА (NOID)", "周期总预算（NOID）"),
        "EDIT AS NEW PROGRAM" => ("ИЗМЕНИТЬ КАК НОВУЮ ПРОГРАММУ", "编辑为新程序"),
        "REVIEW CALL WITHOUT PAYMENT" => ("ПРОВЕРИТЬ ВЫЗОВ БЕЗ ПЛАТЕЖА", "核对无付款调用"),
        "CHECK CANDIDATE BALANCE" => ("ПРОВЕРИТЬ БАЛАНС РЕЗУЛЬТАТА", "检查候选余额"),
        "INCLUSION BLOCK" => ("БЛОК ВКЛЮЧЕНИЯ", "包含区块"),
        "NETWORK FEE (NOID)" => ("КОМИССИЯ СЕТИ (NOID)", "网络手续费（NOID）"),
        "REMAINING BALANCE (NOID)" => ("ОСТАТОК В КОНТРАКТЕ (NOID)", "合约剩余余额（NOID）"),
        "NEXT SAVED COUNTERS" => ("НОВЫЕ ЗНАЧЕНИЯ СЧЁТЧИКОВ", "下一状态计数器"),
        "Contracts become available automatically at the v2 activation block." => ("Контракты станут доступны автоматически с блока активации v2.", "合约将在 v2 激活区块自动启用。"),
        "The budget includes payments and fees. Unused budget does not carry over. The first call after a period expires starts a new period." => ("Бюджет включает платежи и комиссии. Неиспользованная часть не переносится. Первый вызов после истечения периода начинает новый период.", "预算包含付款和手续费。未用预算不结转。周期结束后的首次调用将开始新周期。"),
        "The payee claims a fixed prepaid payment when due. Missed charges do not accumulate. Each successful call starts the next period." => ("Получатель забирает фиксированный предоплаченный платёж с наступлением срока. Пропущенные платежи не накапливаются. Каждый успешный вызов начинает следующий период.", "到期后收款人可领取固定的预付款。错过的付款不累计。每次成功调用都会开始下一周期。"),
        "Fixed tranches unlock on a block schedule. Missed tranches can be claimed one per call. The beneficiary can withdraw the remainder at maturity." => ("Фиксированные транши разблокируются по расписанию блоков. Пропущенные транши можно получать по одному за вызов. С блока полной разблокировки получатель может забрать весь остаток.", "固定分期按区块计划解锁。错过的分期可逐次领取，每次调用领取一期。完全解锁后，受益人可提取余额。"),
        "Write a bounded integer program or edit a template. Creating edited terms makes a new contract; it does not change an existing balance." => ("Напишите короткую целочисленную программу или измените шаблон. Изменённые условия создают новый контракт и не меняют правила уже размещённых средств.", "编写有界整数程序或编辑模板。修改后的条款会创建新合约，不会改变现有余额的规则。"),
        "Two saved u64 counters, two scratch registers, up to 16 instructions. Use decimal strings for counters and constants; amounts in the program are micronoid." => ("Два сохраняемых счётчика u64, два временных регистра, до 16 инструкций. Счётчики и константы задаются десятичными строками; суммы в программе — в микроноидах.", "两个持久化 u64 计数器、两个临时寄存器，最多 16 条指令。计数器和常量使用十进制字符串，程序内金额单位为 micronoid。"),
        "Operations: move, add, subtract, min, max, less_than, equal, assert_equal, assert_less_or_equal. Conditions use the deadline, closing flag, payment flag or a scratch boolean. Arithmetic overflow rejects the call." => ("Операции: move, add, subtract, min, max, less_than, equal, assert_equal, assert_less_or_equal. Условия используют срок, признаки закрытия и платежа либо логическое значение временного регистра. Переполнение отклоняет вызов.", "操作：move、add、subtract、min、max、less_than、equal、assert_equal、assert_less_or_equal。条件可使用期限、关闭标志、付款标志或临时布尔值。算术溢出会拒绝调用。"),
        "Keep an exported copy of your terms. Your wallet key alone cannot restore a custom program. Saved entries can include unfunded or pending terms." => ("Сохраните экспортированную копию условий. Одного ключа кошелька недостаточно для восстановления своей программы. В списке могут быть условия без средств и ожидающие подтверждения.", "请保留导出的条款副本。仅凭钱包密钥无法恢复自定义程序。列表可能包含尚未充值或待确认的条款。"),
        "Checked unsigned 64-bit arithmetic. State and constants are exact decimal integers." => ("Беззнаковая 64-битная арифметика с проверками. Состояние и константы — точные десятичные целые числа.", "经过检查的 64 位无符号运算。状态和常量使用精确的十进制整数。"),
        "The program reads the inclusion height, amounts, fee and call flags. Each condition is enforced by the block proof." => ("Программа читает высоту включения, суммы, комиссию и признаки вызова. Каждое условие проверяется в доказательстве блока.", "程序读取包含高度、金额、手续费和调用标志。每个条件都由区块证明强制执行。"),
        "A successor is saved as a candidate. Check its current balance to establish confirmation." => ("Возможный результат сохранён. Проверьте его текущий баланс, чтобы установить подтверждение.", "后续状态已保存为候选。请检查其当前余额以确认。"),
        "Review the node's exact call result below." => ("Ниже показан точный результат вызова, рассчитанный узлом.", "请核对下方节点计算的精确调用结果。"),
        "No candidate successor." => ("Нет сохранённого возможного результата.", "没有候选后续状态。"),
        "Program exceeds its editor limit." => ("Программа превышает допустимый размер редактора.", "程序超过编辑器大小限制。"),
        "Use a custom_program definition in the editor." => ("В редакторе используйте определение custom_program.", "请在编辑器中使用 custom_program 定义。"),
        "Enter a block height or positive period." => ("Введите высоту блока или положительный период.", "请输入区块高度或正的周期长度。"),
        "The preview does not match the requested call." => ("Предварительный расчёт не соответствует запрошенному вызову.", "预览与请求的调用不符。"),
        "Call details changed. Preview and review the transaction again." => ("Параметры вызова изменились. Пересчитайте и проверьте транзакцию заново.", "调用详情已更改。请重新计算并核对交易。"),
        "The call does not satisfy the program conditions." => ("Вызов не удовлетворяет условиям программы.", "调用不满足程序条件。"),
        "The program result is outside the unsigned 64-bit range. Check the amounts and program." => ("Результат вычисления выходит за диапазон беззнакового 64-битного числа. Проверьте суммы и программу.", "程序计算结果超出无符号 64 位整数范围。请检查金额和程序。"),
        "A program condition must evaluate to 0 or 1." => ("Условие программы должно принимать значение 0 или 1.", "程序条件的值必须是 0 或 1。"),
        "Saved contract list changed. Reload it." => ("Список контрактов изменился. Обновите его.", "已保存的合约列表已更改，请重新加载。"),
        "Saved contract opening changed." => ("Сохранённые условия контракта изменились.", "保存的合约条款已更改。"),
        "Contract program details are incomplete." => ("Данные программы контракта неполные.", "合约程序详情不完整。"),
        "Contract name must be at most 64 characters without control characters." => ("Имя контракта должно содержать не более 64 символов без управляющих знаков.", "合约名称最多 64 个字符，不得包含控制字符。"),
        "Contract library exceeds its limits or has unsupported terms." => ("Хранилище контрактов превышает лимиты или содержит неподдерживаемые условия.", "合约库超出限制或包含不支持的条款。"),
        "Contract library is full. Export older terms before removing them from the list." => ("Список контрактов заполнен. Экспортируйте старые условия, прежде чем убрать их из списка.", "合约库已满。从列表移除旧条款前，请先导出。"),

        "CONTRACTS" => ("КОНТРАКТЫ", "合约"),
        "Create spending rules, share their terms and keep proof of every call." => (
            "Задавайте правила расходования, передавайте условия участникам и сохраняйте доказательства вызовов.",
            "设置支出规则、分享条款并保存每次调用的证明。",
        ),
        "PAYMENT WITH REFUND" => ("ПЛАТЁЖ С ВОЗВРАТОМ", "可退款付款"),
        "TIMELOCKED VAULT" => ("СЕЙФ С БЛОКИРОВКОЙ", "定时金库"),
        "ALLOWANCE WALLET" => ("КОШЕЛЁК С ЛИМИТОМ", "限额钱包"),
        "WAITING FOR THE LOCAL NODE" => ("ОЖИДАНИЕ ЛОКАЛЬНОГО УЗЛА", "正在等待本地节点"),
        "The payee can collect before expiry. Your active address can recover the balance from the expiry block onward." => {
            (
                "Получатель может забрать платёж до срока. Начиная с указанного блока ваш активный адрес может вернуть остаток.",
                "收款人可在到期前领取。从到期区块起，您的当前地址可收回余额。",
            )
        }
        "Your active address owns the vault. Neither you nor another key can withdraw before the unlock block." => {
            (
                "Сейф принадлежит вашему активному адресу. До блока разблокировки средства нельзя вывести ни вашим, ни другим ключом.",
                "金库归您的当前地址所有。在解锁区块之前，任何密钥都无法提款。",
            )
        }
        "The spending key can make capped payments while preserving a reserve. The cap applies to each call. Your active address recovers the balance at the recovery block." => {
            (
                "Расходный ключ может делать платежи в пределах лимита, сохраняя резерв. Лимит действует на каждый вызов. С указанного блока ваш активный адрес может забрать остаток.",
                "支出密钥可在保留储备的前提下支付。限额适用于每次调用。从恢复区块起，您的当前地址可收回余额。",
            )
        }
        "PAYEE ADDRESS" => ("АДРЕС ПОЛУЧАТЕЛЯ", "收款地址"),
        "SPENDING KEY ADDRESS" => ("АДРЕС РАСХОДНОГО КЛЮЧА", "支出密钥地址"),
        "EXPIRY BLOCK" => ("БЛОК ИСТЕЧЕНИЯ СРОКА", "到期区块"),
        "UNLOCK BLOCK" => ("БЛОК РАЗБЛОКИРОВКИ", "解锁区块"),
        "RECOVERY BLOCK" => ("БЛОК ВОЗВРАТА", "恢复区块"),
        "MAXIMUM CALL FEE (NOID)" => (
            "МАКСИМАЛЬНАЯ КОМИССИЯ ВЫЗОВА (NOID)",
            "调用手续费上限（NOID）",
        ),
        "PER-CALL PAYMENT LIMIT (NOID)" => {
            ("ЛИМИТ ПЛАТЕЖА НА ВЫЗОВ (NOID)", "单次支付上限（NOID）")
        }
        "MINIMUM RESERVE (NOID)" => ("МИНИМАЛЬНЫЙ РЕЗЕРВ (NOID)", "最低储备（NOID）"),
        "FIXED PAYMENT RECIPIENT (EMPTY ALLOWS ANY)" => (
            "ФИКСИРОВАННЫЙ ПОЛУЧАТЕЛЬ (ПУСТО — ЛЮБОЙ)",
            "固定收款人（留空则不限）",
        ),
        "CREATE TERMS" => ("СОЗДАТЬ УСЛОВИЯ", "创建条款"),
        "IMPORT TERMS" => ("ИМПОРТ УСЛОВИЙ", "导入条款"),
        "VERIFY RECEIPT FILE" => ("ПРОВЕРИТЬ КВИТАНЦИЮ", "验证凭证文件"),
        "RESTORE SAVED CONTRACT BY ADDRESS" => (
            "ВОССТАНОВИТЬ СОХРАНЁННЫЙ КОНТРАКТ ПО АДРЕСУ",
            "按地址恢复已保存的合约",
        ),
        "RESTORE" => ("ВОССТАНОВИТЬ", "恢复"),
        "CONTRACT TERMS" => ("УСЛОВИЯ КОНТРАКТА", "合约条款"),
        "CREATE NEW TERMS" => ("СОЗДАТЬ НОВЫЕ УСЛОВИЯ", "创建新条款"),
        "BEFORE DEADLINE" => ("ДО УКАЗАННОГО БЛОКА", "期限区块之前"),
        "FROM DEADLINE" => ("С УКАЗАННОГО БЛОКА", "从期限区块起"),
        "PAYMENT" => ("ПЛАТЁЖ", "付款"),
        "WITHDRAWAL" => ("ВЫВОД", "提款"),
        "allowed" => ("разрешён", "允许"),
        "disabled" => ("запрещён", "禁止"),
        "CLOSING RECIPIENT BEFORE DEADLINE" => (
            "ПОЛУЧАТЕЛЬ ПРИ ЗАКРЫТИИ ДО СРОКА",
            "到期前关闭时的收款人",
        ),
        "CLOSING RECIPIENT FROM DEADLINE" => (
            "ПОЛУЧАТЕЛЬ ПРИ ЗАКРЫТИИ С НАСТУПЛЕНИЕМ СРОКА",
            "到期后关闭时的收款人",
        ),
        "The payout cap and reserve apply to continuing calls." => (
            "Лимит платежа и резерв действуют на вызовы, сохраняющие контракт.",
            "支付限额和储备适用于保留合约的调用。",
        ),
        "Continuing payments may go to any recipient." => (
            "Платежи с сохранением контракта разрешены любому получателю.",
            "保留合约的付款可发送给任何收款人。",
        ),
        "Continuing payments use the closing recipient of the active branch." => (
            "Платежи с сохранением контракта направляются получателю при закрытии действующей ветки.",
            "保留合约的付款使用当前期限阶段的关闭收款人。",
        ),
        "Policy only: no additional program conditions." => (
            "Действуют указанные правила; дополнительных условий программы нет.",
            "仅适用上述规则，没有额外的程序条件。",
        ),
        "CUSTOM PROGRAM — ADDITIONAL CONDITIONS APPLY" => (
            "СВОЯ ПРОГРАММА — ЕСТЬ ДОПОЛНИТЕЛЬНЫЕ УСЛОВИЯ",
            "自定义程序 — 有额外条件",
        ),
        "CODE ID" => ("ИДЕНТИФИКАТОР КОДА", "代码标识"),
        "CURRENT STATE" => ("ТЕКУЩЕЕ СОСТОЯНИЕ", "当前状态"),
        "Arithmetic uses a binary field. Constants and State are hexadecimal." => (
            "Арифметика двоичного поля. Константы и состояние записаны в шестнадцатеричной форме.",
            "运算使用二元域。常量和状态以十六进制表示。",
        ),
        "c[0..7]: epoch root halves, fee, first output amount, second output amount, second output owner halves, flags." => (
            "c[0..7]: половины корня эпохи, комиссия, сумма первого выхода, сумма второго выхода, половины владельца второго выхода, флаги.",
            "c[0..7]：纪元根的两半、手续费、第一个输出金额、第二个输出金额、第二个输出所有者的两半、标志。",
        ),
        "The node did not provide the contract program. Update the node and reload the terms before funding." => (
            "Узел не передал программу контракта. Обновите узел и загрузите условия заново перед пополнением.",
            "节点未提供合约程序。请更新节点并重新加载条款后再充值。",
        ),
        "The additional program conditions shown in the terms apply to this funding." => (
            "К этому пополнению применяются дополнительные условия программы, показанные выше.",
            "本次充值受上述额外程序条件约束。",
        ),
        "SAVE / SHARE TERMS" => ("СОХРАНИТЬ / ПЕРЕДАТЬ УСЛОВИЯ", "保存 / 分享条款"),
        "REFRESH BALANCES" => ("ОБНОВИТЬ БАЛАНСЫ", "刷新余额"),
        "No spendable balance for these terms." => (
            "Для этих условий нет доступного остатка.",
            "这些条款下没有可支配余额。",
        ),
        "NEXT PAGE" => ("СЛЕДУЮЩАЯ СТРАНИЦА", "下一页"),
        "AMOUNT TO FUND OR PAY (NOID)" => (
            "СУММА ПОПОЛНЕНИЯ ИЛИ ПЛАТЕЖА (NOID)",
            "充值或支付金额（NOID）",
        ),
        "NETWORK FEE (EMPTY IS AUTOMATIC)" => (
            "КОМИССИЯ СЕТИ (ПУСТО — АВТОМАТИЧЕСКИ)",
            "网络手续费（留空则自动）",
        ),
        "PAYMENT RECIPIENT" => ("ПОЛУЧАТЕЛЬ ПЛАТЕЖА", "付款收款人"),
        "REVIEW FUNDING" => ("ПРОВЕРИТЬ ПОПОЛНЕНИЕ", "核对充值"),
        "REVIEW PAYMENT" => ("ПРОВЕРИТЬ ПЛАТЁЖ", "核对付款"),
        "REVIEW WITHDRAWAL" => ("ПРОВЕРИТЬ ВЫВОД", "核对提款"),
        "CONFIRMED CALL TRANSACTION ID" => (
            "ID ТРАНЗАКЦИИ ПОДТВЕРЖДЁННОГО ВЫЗОВА",
            "已确认调用的交易 ID",
        ),
        "SAVE VERIFIED RECEIPT" => ("СОХРАНИТЬ ПРОВЕРЕННУЮ КВИТАНЦИЮ", "保存已验证凭证"),
        "REVIEW TRANSACTION" => ("ПРОВЕРКА ТРАНЗАКЦИИ", "核对交易"),
        "CONFIRM" => ("ПОДТВЕРДИТЬ", "确认"),
        "CANCEL" => ("ОТМЕНА", "取消"),
        "Field is too long." => ("Слишком длинное значение поля.", "输入内容过长。"),
        "Active address changed. Review the transaction again." => (
            "Активный адрес изменился. Проверьте транзакцию заново.",
            "当前地址已更改。请重新核对交易。",
        ),
        "Contract authority or deadline branch changed. Review the transaction again." => (
            "Изменился действующий ключ или наступил срок контракта. Проверьте транзакцию заново.",
            "合约授权密钥或期限阶段已更改。请重新核对交易。",
        ),
        "Enter a block height." => ("Введите высоту блока.", "请输入区块高度。"),
        "Choose a future block height." => {
            ("Выберите будущую высоту блока.", "请选择未来的区块高度。")
        }
        "No next page." => ("Следующей страницы нет.", "没有下一页。"),
        "Enter a transaction ID." => ("Введите ID транзакции.", "请输入交易 ID。"),
        "Import the terms used for this call." => (
            "Импортируйте условия, использованные для этого вызова.",
            "请导入此调用使用的条款。",
        ),
        "Create or import contract terms first." => (
            "Сначала создайте или импортируйте условия контракта.",
            "请先创建或导入合约条款。",
        ),
        "Select a funded contract first." => (
            "Сначала выберите пополненный контракт.",
            "请先选择已充值的合约。",
        ),
        "This action is unavailable to the active address at the next block." => (
            "Это действие недоступно активному адресу в следующем блоке.",
            "当前地址在下一区块无权执行此操作。",
        ),
        "Fee exceeds the contract limit." => (
            "Комиссия превышает лимит контракта.",
            "手续费超过合约上限。",
        ),
        "This contract does not allow that action at the next block." => (
            "Условия контракта запрещают это действие в следующем блоке.",
            "合约条款不允许在下一区块执行此操作。",
        ),
        "The network fee exceeds this contract's limit." => (
            "Комиссия сети превышает лимит контракта.",
            "网络手续费超过此合约的上限。",
        ),
        "This payment would leave less than the contract's minimum reserve." => (
            "После этого платежа останется меньше минимального резерва контракта.",
            "付款后的余额将低于合约的最低储备。",
        ),
        "The recipient differs from the contract terms." => (
            "Получатель не соответствует условиям контракта.",
            "收款人与合约条款不符。",
        ),
        "The active address cannot authorize this contract at the next block." => (
            "Активный адрес не может подтвердить вызов этого контракта в следующем блоке.",
            "当前地址无权在下一区块授权此合约调用。",
        ),
        "The selected balance has changed. Refresh and review the transaction again." => (
            "Выбранный остаток изменился. Обновите данные и проверьте транзакцию заново.",
            "所选余额已变化。请刷新并重新检查交易。",
        ),
        "Payment exceeds the per-call limit." => (
            "Платёж превышает лимит одного вызова.",
            "付款超过单次调用限额。",
        ),
        "Enter a payment recipient." => ("Введите адрес получателя платежа.", "请输入付款收款人。"),
        "Connect to a v2 node to use contracts." => (
            "Для работы с контрактами подключитесь к узлу v2.",
            "请连接 v2 节点以使用合约。",
        ),
        "Public contract terms saved. Share this file with the other participant." => (
            "Публичные условия контракта сохранены. Передайте файл другому участнику.",
            "公开合约条款已保存。请将此文件分享给另一位参与者。",
        ),
        "Contract artifact is not a regular file within its size limit." => (
            "Файл контракта имеет недопустимый тип или размер.",
            "合约文件类型或大小不符合要求。",
        ),
        "Contract artifact exceeds its size limit." => (
            "Файл контракта превышает допустимый размер.",
            "合约文件超过大小限制。",
        ),
        "This file has no current contract terms." => (
            "В файле нет текущих условий контракта.",
            "文件中没有当前合约条款。",
        ),
        "Receipt verification did not match the requested call." => (
            "Проверенная квитанция не соответствует запрошенному вызову.",
            "验证的凭证与请求的调用不一致。",
        ),
        "Contract receipt did not verify." => (
            "Квитанция контракта не прошла проверку.",
            "合约凭证验证失败。",
        ),
        "Receipt exceeds its file limit." => (
            "Квитанция превышает допустимый размер файла.",
            "凭证超过文件大小限制。",
        ),
        "Node returned an invalid transaction ID." => (
            "Узел вернул некорректный ID транзакции.",
            "节点返回了无效的交易 ID。",
        ),
        "Invalid contract status." => ("Некорректное состояние контракта.", "合约状态无效。"),
        _ => return dynamic(language, source),
    };
    Some(select(language, source, pair.0, pair.1))
}

fn select(language: Language, english: &str, russian: &str, chinese: &str) -> String {
    match language {
        Language::English => english,
        Language::Russian => russian,
        Language::Chinese => chinese,
    }
    .to_owned()
}

fn dynamic(language: Language, source: &str) -> Option<String> {
    if let Some(height) = source
        .strip_prefix("Current block: ")
        .and_then(|s| s.strip_suffix(". Contract deadlines use block heights."))
    {
        return Some(select(
            language,
            source,
            &format!("Текущий блок: {height}. Сроки контрактов задаются высотой блока."),
            &format!("当前区块：{height}。合约期限按区块高度计算。"),
        ));
    }
    for (prefix, ru, zh) in [
        (
            "Authority before block ",
            "Ключ до блока",
            "此区块之前的授权密钥",
        ),
        (
            "Authority from block ",
            "Ключ начиная с блока",
            "从此区块起的授权密钥",
        ),
    ] {
        if let Some((height, address)) =
            source.strip_prefix(prefix).and_then(|s| s.split_once(": "))
        {
            return Some(select(
                language,
                source,
                &format!("{ru} {height}: {address}"),
                &format!("{zh} {height}：{address}"),
            ));
        }
    }
    if let Some((fee, tail)) = source
        .strip_prefix("Maximum fee: ")
        .and_then(|s| s.split_once(" NOID · Per-call payout cap: "))
    {
        if let Some((cap, reserve)) = tail
            .strip_suffix(" NOID")
            .and_then(|s| s.split_once(" NOID · Reserve: "))
        {
            return Some(select(
                language,
                source,
                &format!(
                    "Макс. комиссия: {fee} NOID · Лимит на вызов: {cap} NOID · Резерв: {reserve} NOID"
                ),
                &format!("手续费上限：{fee} NOID · 单次限额：{cap} NOID · 储备：{reserve} NOID"),
            ));
        }
    }
    if let Some(address) = source.strip_prefix("Recipient for a closing call at the next block: ") {
        return Some(select(
            language,
            source,
            &format!("Получатель при закрытии в следующем блоке: {address}"),
            &format!("下一区块关闭合约时的收款人：{address}"),
        ));
    }
    if let Some(height) = source.strip_prefix("Funded contracts at block ") {
        return Some(select(
            language,
            source,
            &format!("Пополненные контракты на блоке {height}"),
            &format!("区块 {height} 的已充值合约"),
        ));
    }
    if let Some((amount, tail)) = source.split_once(" NOID · position ") {
        if let Some((slot, creation)) = tail.split_once(" · creation ") {
            return Some(select(
                language,
                source,
                &format!("{amount} NOID · позиция {slot} · создание {creation}"),
                &format!("{amount} NOID · 位置 {slot} · 创建编号 {creation}"),
            ));
        }
    }
    if let Some(txid) = source
        .strip_prefix("Submitted: ")
        .and_then(|s| s.strip_suffix(". Awaiting confirmation; refresh to read current balances."))
    {
        return Some(select(
            language,
            source,
            &format!("Отправлено: {txid}. Ожидается подтверждение; обновите балансы."),
            &format!("已提交：{txid}。正在等待确认；请刷新余额。"),
        ));
    }
    if let Some(txid) = source
        .strip_prefix("Verified receipt saved for transaction ")
        .and_then(|s| s.strip_suffix('.'))
    {
        return Some(select(
            language,
            source,
            &format!("Проверенная квитанция сохранена для транзакции {txid}."),
            &format!("交易 {txid} 的已验证凭证已保存。"),
        ));
    }
    if let Some((balance, address)) = source
        .strip_prefix("Close the selected contract and send its ")
        .and_then(|s| s.strip_suffix('.'))
        .and_then(|s| s.split_once(" NOID balance, less the network fee, to "))
    {
        return Some(select(
            language,
            source,
            &format!(
                "Закрыть выбранный контракт и отправить его остаток {balance} NOID за вычетом комиссии сети на {address}."
            ),
            &format!("关闭选定合约，将其 {balance} NOID 余额扣除网络手续费后发送至 {address}。"),
        ));
    }
    if let Some((amount, address)) = source
        .strip_prefix("Pay ")
        .and_then(|s| s.strip_suffix(". The remaining balance stays under the contract terms."))
        .and_then(|s| s.split_once(" NOID to "))
    {
        return Some(select(
            language,
            source,
            &format!("Перевести {amount} NOID на {address}. Остаток сохраняет условия контракта."),
            &format!("向 {address} 支付 {amount} NOID。余额继续受合约条款约束。"),
        ));
    }
    if let Some((address, tail)) = source
        .strip_prefix("Fund ")
        .and_then(|s| s.split_once(" with "))
    {
        if let Some((amount, tail)) = tail.split_once(" NOID from ") {
            if let Some((sender, fee)) = tail
                .strip_suffix('.')
                .and_then(|s| s.split_once(". Network fee: "))
            {
                let ru_fee = if fee == "automatic" {
                    "автоматически"
                } else {
                    fee
                };
                let zh_fee = if fee == "automatic" { "自动" } else { fee };
                return Some(select(
                    language,
                    source,
                    &format!(
                        "Пополнить {address} на {amount} NOID с адреса {sender}. Комиссия сети: {ru_fee}."
                    ),
                    &format!("从 {sender} 向 {address} 充值 {amount} NOID。网络手续费：{zh_fee}。"),
                ));
            }
        }
    }
    if let Some((txid, height)) = source.strip_prefix("Verified on this node's selected chain: transaction ").and_then(|s| s.strip_suffix(". This proves the recorded call; refresh current balances to check whether its successor remains spendable.")).and_then(|s| s.split_once(" in block ")) {
        return Some(select(language, source, &format!("Проверено в выбранной цепи этого узла: транзакция {txid} в блоке {height}. Это доказательство записанного вызова; обновите балансы, чтобы проверить доступность его результата."), &format!("已在此节点选定的链上验证：区块 {height} 中的交易 {txid}。这证明了已记录的调用；请刷新余额以检查后续输出是否仍可支配。")));
    }
    for (prefix, ru, zh) in [
        ("BLOCK #", "БЛОК #", "区块 #"),
        ("DEPOSIT ", "ПОПОЛНЕНИЕ ", "充值 "),
        ("Checked at block ", "Проверено на блоке ", "已检查，区块 "),
        (
            "Other states checked at block ",
            "Другие состояния проверены на блоке ",
            "其他状态已检查，区块 ",
        ),
        (
            "OPEN AVAILABLE STATE · ",
            "ОТКРЫТЬ ДОСТУПНЫЙ ОСТАТОК · ",
            "打开可用状态 · ",
        ),
    ] {
        if let Some(value) = source.strip_prefix(prefix) {
            return Some(select(
                language,
                source,
                &format!("{ru}{value}"),
                &format!("{zh}{value}"),
            ));
        }
    }
    None
}
