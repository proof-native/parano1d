use noid_poseidon2b::primitives::Address;
use noid_tx::{
    experimental_object::{applications::*, ObjectError},
    TxInput, TxOutput,
};

fn input() -> TxInput {
    TxInput {
        slot_index: 1,
        amount: 10_000,
        creation_id: 3,
    }
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
