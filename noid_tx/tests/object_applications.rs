use noid_poseidon2b::primitives::Address;
use noid_tx::{
    experimental_object::{
        applications::*, integer_program::unpack_state, ObjectError, ObjectOpening,
    },
    TxInput, TxOutput, TxPage,
};

fn input() -> TxInput {
    TxInput {
        slot_index: 1,
        amount: 10_000,
        creation_id: 3,
    }
}

fn payment(
    object: &ObjectOpening,
    input: TxInput,
    height: u64,
    fee: u64,
    amount: u64,
) -> Result<TxPage, ObjectError> {
    object.build_payment(
        input,
        input.slot_index + 3,
        fee,
        [4; 32],
        height,
        TxOutput {
            slot_index: input.slot_index + 4,
            amount,
            owner: object.claim_recipient,
        },
    )
}

fn successor(object: &ObjectOpening, page: &TxPage, height: u64) -> (ObjectOpening, TxInput) {
    let checked = object.check_call(page, height).unwrap();
    let next = object.successor(checked.next);
    assert_eq!(page.body.outputs[0].owner, next.root());
    (
        next,
        TxInput {
            slot_index: page.body.outputs[0].slot_index,
            amount: page.body.outputs[0].amount,
            creation_id: page.body.inputs[0].creation_id + 2,
        },
    )
}

fn budget_terms() -> PeriodBudget {
    PeriodBudget {
        start_height: 100,
        period_blocks: 10,
        budget: 100,
        recover_at: 150,
        max_fee: 10,
        max_payout: 100,
        min_retained: 20,
    }
}

#[test]
fn period_budget_counts_fees_and_cannot_be_reset_by_another_call() {
    let spender = Address([1; 32]);
    let recovery = Address([2; 32]);
    let (mut object, mut coin) = (
        period_budget_wallet(spender, recovery, None, budget_terms()).unwrap(),
        input(),
    );
    assert!(payment(&object, coin, 99, 5, 55).is_err());
    let page = payment(&object, coin, 100, 5, 55).unwrap();
    (object, coin) = successor(&object, &page, 100);
    assert_eq!(unpack_state(object.state), [40, 110]);
    assert!(payment(&object, coin, 109, 0, 41).is_err());
    let page = payment(&object, coin, 109, 10, 30).unwrap();
    (object, coin) = successor(&object, &page, 109);
    assert_eq!(unpack_state(object.state), [0, 110]);
    // Even calls without a payout consume their fee from the same budget.
    assert!(object
        .build_call(coin, coin.slot_index + 3, 1, [4; 32], 109, false)
        .is_err());
    let page = payment(&object, coin, 110, 10, 90).unwrap();
    (object, coin) = successor(&object, &page, 110);
    assert_eq!(unpack_state(object.state), [0, 120]);
    assert!(payment(&object, coin, 110, 0, 1).is_err());
    let page = payment(&object, coin, 135, 10, 90).unwrap();
    (object, coin) = successor(&object, &page, 135);
    assert_eq!(unpack_state(object.state), [0, 145]);
    assert!(payment(&object, coin, 144, 0, 1).is_err());
    assert!(object
        .build_call(coin, coin.slot_index + 3, 0, [4; 32], 149, true)
        .is_err());
    let close = object
        .build_call(coin, coin.slot_index + 3, 10, [4; 32], 150, true)
        .unwrap();
    assert_eq!(close.body.outputs[0].owner, recovery);
    assert_eq!(object.check_call(&close, 150).unwrap().authority, recovery);
    assert!(payment(&object, coin, 150, 0, 1).is_err());
}

#[test]
fn recurring_payments_require_due_height_exact_amount_and_do_not_accumulate() {
    let payer = Address([1; 32]);
    let payee = Address([2; 32]);
    let terms = RecurringPayment {
        first_due_height: 100,
        period_blocks: 10,
        payment: 50,
        recover_at: 180,
        max_fee: 5,
    };
    let (mut object, mut coin) = (recurring_payment(payer, payee, terms).unwrap(), input());
    assert!(payment(&object, coin, 99, 5, 50).is_err());
    for amount in [0, 49, 51] {
        assert!(payment(&object, coin, 100, 5, amount).is_err());
    }
    assert!(object.build_call(coin, 4, 5, [4; 32], 100, false).is_err());
    let first = payment(&object, coin, 101, 5, 50).unwrap();
    assert_eq!(object.check_call(&first, 101).unwrap().authority, payee);
    (object, coin) = successor(&object, &first, 101);
    assert_eq!(unpack_state(object.state), [1, 111]);
    for height in [101, 110] {
        assert!(payment(&object, coin, height, 5, 50).is_err());
    }
    let page = payment(&object, coin, 160, 5, 50).unwrap();
    (object, coin) = successor(&object, &page, 160);
    assert_eq!(unpack_state(object.state), [2, 170]);
    assert!(payment(&object, coin, 169, 5, 50).is_err());
    let too_small = TxInput { amount: 59, ..coin };
    assert_eq!(
        payment(&object, too_small, 170, 5, 50),
        Err(ObjectError::ReserveLimit)
    );
    let page = payment(&object, coin, 170, 5, 50).unwrap();
    (object, coin) = successor(&object, &page, 170);
    assert!(payment(&object, coin, 180, 5, 50).is_err());
    let close = object
        .build_call(coin, coin.slot_index + 3, 5, [4; 32], 180, true)
        .unwrap();
    assert_eq!(close.body.outputs[0].owner, payer);
    assert_eq!(object.check_call(&close, 180).unwrap().authority, payer);
}

#[test]
fn vesting_preserves_missed_tranches_and_releases_remainder_at_maturity() {
    let beneficiary = Address([1; 32]);
    let terms = TrancheVesting {
        first_unlock_height: 100,
        period_blocks: 10,
        tranche_amount: 50,
        mature_at: 150,
        max_fee: 5,
    };
    let (mut object, mut coin) = (tranche_vesting(beneficiary, terms).unwrap(), input());
    assert!(payment(&object, coin, 99, 5, 50).is_err());
    for count in 1..=3 {
        let page = payment(&object, coin, 129, 5, 50).unwrap();
        (object, coin) = successor(&object, &page, 129);
        assert_eq!(unpack_state(object.state), [count * 50, 100 + count * 10]);
    }
    assert!(payment(&object, coin, 129, 5, 50).is_err());
    for count in 4..=5 {
        let page = payment(&object, coin, 149, 5, 50).unwrap();
        (object, coin) = successor(&object, &page, 149);
        assert_eq!(unpack_state(object.state), [count * 50, 100 + count * 10]);
    }
    assert!(payment(&object, coin, 149, 5, 50).is_err());
    assert!(object
        .build_call(coin, coin.slot_index + 3, 5, [4; 32], 149, true)
        .is_err());
    let close = object
        .build_call(coin, coin.slot_index + 3, 5, [4; 32], 150, true)
        .unwrap();
    assert_eq!(close.body.outputs[0].owner, beneficiary);
    assert_eq!(close.body.outputs[0].amount, 10_000 - 5 * 55 - 5);
}

#[test]
fn schedules_reject_empty_or_wrapping_terms_and_recovery_skips_counter_overflow() {
    let owner = Address([1; 32]);
    let terms = budget_terms();
    for (changed, expected) in [
        (
            PeriodBudget {
                period_blocks: 0,
                ..terms
            },
            ScheduleError::EmptyPeriod,
        ),
        (
            PeriodBudget { budget: 0, ..terms },
            ScheduleError::EmptyAmount,
        ),
        (
            PeriodBudget {
                recover_at: 100,
                ..terms
            },
            ScheduleError::DeadlineBeforeStart,
        ),
        (
            PeriodBudget {
                recover_at: u64::MAX,
                ..terms
            },
            ScheduleError::HeightOverflow,
        ),
        (
            PeriodBudget {
                max_fee: 101,
                ..terms
            },
            ScheduleError::FeeExceedsBudget,
        ),
    ] {
        assert_eq!(
            period_budget_wallet(owner, owner, None, changed),
            Err(expected)
        );
    }
    let mut object = recurring_payment(
        owner,
        owner,
        RecurringPayment {
            first_due_height: 100,
            period_blocks: 10,
            payment: 50,
            recover_at: 180,
            max_fee: 5,
        },
    )
    .unwrap();
    object.state = noid_tx::experimental_object::integer_program::pack_state([u64::MAX, 100]);
    assert!(payment(&object, input(), 100, 5, 50).is_err());
    assert!(object.build_call(input(), 4, 5, [4; 32], 180, true).is_ok());
}

#[test]
fn refundable_payment_cannot_be_drained_by_a_continuation_or_excess_fee() {
    let payer = Address([1; 32]);
    let payee = Address([2; 32]);
    let contract = refundable_payment(payer, payee, 50, 100);
    assert_eq!(
        contract.build_call(input(), 2, 100, [3; 32], 49, false),
        Err(ObjectError::Policy)
    );
    assert_eq!(
        contract.build_call(input(), 2, 101, [3; 32], 49, true),
        Err(ObjectError::FeeLimit)
    );
    for (height, recipient) in [(49, payee), (50, payer), (51, payer)] {
        let page = contract
            .build_call(input(), 2, 100, [3; 32], height, true)
            .unwrap();
        assert_eq!(page.body.outputs[0].owner, recipient);
        assert_eq!(page.body.outputs[0].amount, 9_900);
        let check = contract.check_call(&page, height).unwrap();
        assert_eq!(check.authority, recipient);
    }
    let early = contract
        .build_call(input(), 2, 100, [3; 32], 49, true)
        .unwrap();
    assert!(contract.check_call(&early, 50).is_err());
}

#[test]
fn vault_owner_has_no_spending_path_before_unlock_height() {
    let owner = Address([1; 32]);
    let contract = timelocked_vault(owner, 50, 100);
    for height in [0, 1, 49] {
        for terminal in [false, true] {
            assert_eq!(
                contract.build_call(input(), 2, 0, [3; 32], height, terminal),
                Err(ObjectError::Policy)
            );
        }
    }
    let unlocked = contract
        .build_call(input(), 2, 100, [3; 32], 50, true)
        .unwrap();
    assert_eq!(unlocked.body.outputs[0].owner, owner);
    assert!(contract.check_call(&unlocked, 49).is_err());
    assert!(contract.check_call(&unlocked, 50).is_ok());
}

#[test]
fn spending_key_obeys_reserve_recipient_and_payment_limit_and_cannot_close() {
    let spender = Address([1; 32]);
    let recovery = Address([2; 32]);
    let payee = Address([3; 32]);
    let contract = allowance_wallet(spender, recovery, Some(payee), 50, 100, 500, 9_400);
    let payout = TxOutput {
        slot_index: 3,
        amount: 500,
        owner: payee,
    };
    let page = contract
        .build_payment(input(), 2, 100, [4; 32], 49, payout)
        .unwrap();
    let checked = contract.check_call(&page, 49).unwrap();
    assert_eq!(checked.authority, spender);
    assert_eq!(page.body.outputs[0].amount, 9_400);
    assert_eq!(
        page.body.outputs[0].owner,
        contract.successor(checked.next).root()
    );
    assert!(contract
        .build_call(input(), 2, 100, [4; 32], 49, true)
        .is_err());
    assert!(contract
        .build_payment(input(), 2, 100, [4; 32], 50, payout)
        .is_err());
    let recovered = contract
        .build_call(input(), 2, 100, [4; 32], 50, true)
        .unwrap();
    assert_eq!(recovered.body.outputs[0].owner, recovery);
    assert_eq!(
        contract.check_call(&recovered, 50).unwrap().authority,
        recovery
    );
    for wrong in [
        TxOutput {
            amount: 501,
            ..payout
        },
        TxOutput {
            owner: Address([9; 32]),
            ..payout
        },
    ] {
        assert!(contract
            .build_payment(input(), 2, 100, [4; 32], 49, wrong)
            .is_err());
    }
    let smaller = TxInput {
        amount: 9_999,
        ..input()
    };
    assert_eq!(
        contract.build_payment(smaller, 2, 100, [4; 32], 49, payout),
        Err(ObjectError::ReserveLimit)
    );
    assert_eq!(
        contract.build_payment(input(), 2, 101, [4; 32], 49, payout),
        Err(ObjectError::FeeLimit)
    );
    // The reserve is committed by the successor root, so it cannot disappear
    // on the second call while keeping the original policy.
    let successor = contract.successor(checked.next);
    let next_input = TxInput {
        slot_index: 2,
        amount: 9_400,
        creation_id: 4,
    };
    let next_payout = TxOutput {
        slot_index: 4,
        ..payout
    };
    assert_eq!(
        successor.build_payment(next_input, 5, 0, [4; 32], 49, next_payout),
        Err(ObjectError::ReserveLimit)
    );
}

#[test]
fn optional_recipient_freedom_does_not_disable_amount_limits() {
    let contract = allowance_wallet(
        Address([1; 32]),
        Address([2; 32]),
        None,
        50,
        100,
        500,
        9_400,
    );
    let payout = TxOutput {
        slot_index: 3,
        amount: 500,
        owner: Address([9; 32]),
    };
    assert!(contract
        .build_payment(input(), 2, 100, [4; 32], 49, payout)
        .is_ok());
    assert!(contract
        .build_payment(
            input(),
            2,
            100,
            [4; 32],
            49,
            TxOutput {
                amount: 501,
                ..payout
            }
        )
        .is_err());
}
