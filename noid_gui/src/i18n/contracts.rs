// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

use crate::model::Language;

pub(super) fn translate(language: Language, source: &str) -> Option<String> {
    let pair = match source {
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
    None
}
