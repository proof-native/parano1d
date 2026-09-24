use noid_core::Block128;
use noid_poseidon2b::primitives::Address;
use noid_tx::{experimental_object::*, *};

fn opening() -> ObjectOpening {
    ObjectOpening {
        program: std::array::from_fn(|step| {
            [Block128((step % 4) as u128), Block128(step as u128 + 7)]
        }),
        state: Block128(11),
        claim_authority: Address([1; 32]),
        recovery_authority: Address([2; 32]),
        deadline: 10,
        claim_recipient: Address([3; 32]),
        recovery_recipient: Address([4; 32]),
        rules: noid_tx::experimental_object::ObjectRules {
            max_fee: u64::MAX,
            min_retained: 0,
            max_payout: u64::MAX,
            modes: 31,
        },
    }
}

fn call(object: &ObjectOpening, height: u64, terminal: bool) -> TxPage {
    object
        .build_call(
            TxInput {
                slot_index: 1,
                amount: 1000,
                creation_id: 7,
            },
            2,
            10,
            [9; 32],
            height,
            terminal,
        )
        .unwrap()
}

#[test]
fn fixed_wire_roundtrip_and_every_truncation() {
    let object = opening();
    let bytes = object.to_bytes().unwrap();
    assert_eq!(bytes.len(), 443);
    assert_eq!(ObjectOpening::from_bytes(&bytes), Ok(object.clone()));
    for cut in 0..bytes.len() {
        assert!(ObjectOpening::from_bytes(&bytes[..cut]).is_err());
    }
    let mut over = bytes.to_vec();
    over.extend_from_slice(&[0; 32]); // ninth instruction / extension is not canonical
    assert!(ObjectOpening::from_bytes(&over).is_err());
    let mut bad = bytes;
    bad[8] = 3;
    assert_eq!(ObjectOpening::from_bytes(&bad), Err(ObjectError::Version));
    let mut bad = bytes;
    bad[10] = 8;
    assert_eq!(
        ObjectOpening::from_bytes(&bad),
        Err(ObjectError::Opcode { step: 0 })
    );

    let intent = ObjectIntent {
        opening: object.clone(),
        spend: PagedSpendIntent::new(vec![call(&object, 9, false)], vec![5; 20]).unwrap(),
    };
    let bytes = intent.to_bytes().unwrap();
    assert_eq!(bytes.len(), 781 + 20);
    assert_eq!(ObjectIntent::from_bytes(&bytes), Ok(intent));
    for cut in 0..bytes.len() {
        assert!(ObjectIntent::from_bytes(&bytes[..cut]).is_err());
    }
    let mut bad = bytes.clone();
    bad.push(0);
    assert!(ObjectIntent::from_bytes(&bad).is_err());
    let mut bad = bytes.clone();
    bad[INTENT_PREFIX_BYTES + 1] = 2;
    assert!(ObjectIntent::from_bytes(&bad).is_err());
    let mut bad = bytes;
    let offset = INTENT_PREFIX_BYTES + 3 + TX_BODY_WIRE_SIZE;
    bad[offset..offset + 4]
        .copy_from_slice(&((MAX_TX_AUTHORIZATION_BYTES + 1) as u32).to_le_bytes());
    assert!(ObjectIntent::from_bytes(&bad).is_err());
}

#[test]
fn continuing_and_terminal_match_deadline_boundary() {
    let object = opening();
    let page = call(&object, 9, false);
    assert_eq!(page.to_bytes().unwrap().len(), 323);
    let checked = object.check_call(&page, 9).unwrap();
    assert_eq!(checked.authority, object.claim_authority);
    assert_eq!(
        page.body.outputs[0].owner,
        object.successor(checked.next).root()
    );
    for height in [9, 10, 11] {
        let page = call(&object, height, true);
        assert_eq!(page.body.outputs[0].owner, object.recipient_at(height));
        assert_eq!(
            object.check_call(&page, height).unwrap().authority,
            object.authority_at(height)
        );
    }
    assert!(object.check_call(&call(&object, 9, true), 10).is_err());
    assert!(object.check_call(&call(&object, 10, true), 9).is_err());
}

#[test]
fn every_opening_byte_binds_root_or_rejects() {
    let object = opening();
    let bytes = object.to_bytes().unwrap();
    for index in 0..bytes.len() {
        let mut bad = bytes;
        bad[index] ^= 1;
        if let Ok(other) = ObjectOpening::from_bytes(&bad) {
            assert_ne!(other.root(), object.root(), "unbound byte {index}");
        }
    }
}

#[test]
fn invalid_program_predecessor_successor_context_and_effects_reject() {
    let object = opening();
    let honest = call(&object, 9, false);
    let mut wrong = object.clone();
    wrong.program[0][1] = Block128(999);
    assert_eq!(wrong.check_call(&honest, 9), Err(ObjectError::OldObject));
    wrong = object.clone();
    wrong.program[0][0] = Block128(8);
    assert!(matches!(
        wrong.check_call(&honest, 9),
        Err(ObjectError::Opcode { .. })
    ));
    assert!(wrong.funding_output(2, 1000).is_err());
    wrong = object.clone();
    wrong.state = Block128(12);
    assert_eq!(wrong.check_call(&honest, 9), Err(ObjectError::OldObject));
    let mut bad = honest.clone();
    bad.body.outputs[0].owner.0[0] ^= 1;
    assert_eq!(object.check_call(&bad, 9), Err(ObjectError::Successor));
    // Change the step-3 amount context while preserving ordinary balance.
    let mut bad = honest.clone();
    bad.body.outputs[0].amount -= 1;
    bad.body.fee += 1;
    assert_eq!(object.check_call(&bad, 9), Err(ObjectError::Successor));
    let mut bad = honest.clone();
    bad.body.outputs[0].amount += 1;
    assert!(object.check_call(&bad, 9).is_err());
    let mut bad = call(&object, 9, true);
    bad.body.outputs[0].owner = object.recovery_recipient;
    assert_eq!(object.check_call(&bad, 9), Err(ObjectError::Recipient));
    let mut bad = call(&object, 9, true);
    bad.body.outputs[0].amount -= 1;
    bad.body.outputs[1] = TxOutput {
        slot_index: 3,
        amount: 1,
        owner: Address([8; 32]),
    };
    bad.body.validity_bitmap |= output_bitmap_bit(1);
    assert_eq!(object.check_call(&bad, 9), Err(ObjectError::Shape));
}

#[test]
fn ordinary_body_and_capsule_carrier_stay_distinct() {
    let object = opening();
    let page = call(&object, 9, false);
    let page_bytes = page.to_bytes().unwrap();
    assert!(TxBody::from_bytes(&page_bytes).is_err());
    let intent = ObjectIntent {
        opening: object,
        spend: PagedSpendIntent::new(vec![page], vec![]).unwrap(),
    };
    assert!(PagedSpendIntent::from_bytes(&intent.to_bytes().unwrap()).is_err());
}

#[test]
fn assertion_and_load_opcodes_use_the_committed_transaction_contexts() {
    let mut object = opening();
    object.program = [[Block128(0); 2]; PROGRAM_STEPS];
    let page = call(&object, 9, false);
    let contexts = transaction_contexts(&page.body);
    // Load a transaction field, check it, load an immediate, check that state,
    // then require the actual payment amount and load its recipient field.
    object.program[0] = [Block128(6), Block128(0)];
    object.program[1] = [Block128(5), contexts[0]];
    object.program[2] = [Block128(7), Block128(123)];
    object.program[3] = [Block128(5), Block128(123)];
    object.program[4] = [Block128(4), contexts[4]];
    object.program[5] = [Block128(6), Block128(0)];
    assert_eq!(object.execute(&page.body), Ok(contexts[5]));
    let mut wrong = object.clone();
    wrong.program[3][1] = Block128(124);
    assert_eq!(
        wrong.execute(&page.body),
        Err(ObjectError::Assertion { step: 3 })
    );
    let mut wrong_body = page.body;
    wrong_body.outputs[1].amount ^= 1;
    assert_eq!(
        object.execute(&wrong_body),
        Err(ObjectError::Assertion { step: 4 })
    );
    object.rules.modes |= 128;
    assert_eq!(object.validate(), Err(ObjectError::Policy));
}
