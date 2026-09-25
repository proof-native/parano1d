//! Research-only binding experiment for a minimal proof-native contract ABI.
//!
//! This does not change the transaction wire format or consensus.  It checks
//! whether one immutable eight-step program and one mutable field of object
//! State can be committed into the existing owner fields, while every value
//! exposed to the program is already bound by the canonical Tx8x2 hash.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use noid_core::{Block128, CanonicalSerialize, TowerField};
use noid_poseidon2b::native::compression::Poseidon2bSponge;
use noid_poseidon2b::native::domain::{capacity_iv, DomainTag};
use noid_poseidon2b::primitives::{hash_utxo_leaf, Address};
use noid_tx::body_hash::{
    body_hash_leaves, TX8X2_LEAF_EPOCH_ANCHOR, TX8X2_LEAF_FEE, TX8X2_LEAF_FLAGS,
    TX8X2_LEAF_OUTPUT0_DATA, TX8X2_LEAF_OUTPUT1_DATA, TX8X2_LEAF_OUTPUT1_OWNER,
};
use noid_tx::{
    output_bitmap_bit, pack_amount_creation_id, TxBody, TxInput, TxOutput, TX_BODY_WIRE_SIZE,
    TX_INPUTS, TX_OUTPUTS,
};
use serde_json::{json, Value};

const PROGRAM_STEPS: usize = 8;
const PROGRAM_CELLS: usize = PROGRAM_STEPS * 2;
const CODE_DOMAIN: DomainTag = DomainTag::new(b"CNTCODE_");
const OBJECT_DOMAIN: DomainTag = DomainTag::new(b"CNTOBJ__");
const OBJECT_VERSION: Block128 = Block128(2);

fn fixture_field(index: usize, domain: u128) -> Block128 {
    let value = domain
        .wrapping_mul(index as u128 + 1)
        .rotate_left(((index * 17 + 9) % 127) as u32)
        ^ (index as u128 + 29).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let field = Block128::from(value);
    if field == Block128::ZERO {
        Block128::ONE
    } else {
        field
    }
}

fn fields_from_bytes(bytes: [u8; 32]) -> [Block128; 2] {
    [
        Block128::from(u128::from_le_bytes(bytes[..16].try_into().unwrap())),
        Block128::from(u128::from_le_bytes(bytes[16..].try_into().unwrap())),
    ]
}

fn address_from_fields(fields: [Block128; 2]) -> Address {
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(&fields[0].to_bytes());
    bytes[16..].copy_from_slice(&fields[1].to_bytes());
    Address(bytes)
}

fn code_digest(program: &[[Block128; 2]; PROGRAM_STEPS]) -> [Block128; 2] {
    let mut sponge = Poseidon2bSponge::with_iv(capacity_iv(CODE_DOMAIN));
    for instruction in program {
        sponge.absorb_pair(instruction[0], instruction[1]);
    }
    fields_from_bytes(sponge.finalize_no_pad())
}

fn object_root(
    code: [Block128; 2],
    state: Block128,
    controller: [Block128; 2],
    version: Block128,
) -> Address {
    let mut sponge = Poseidon2bSponge::with_iv(capacity_iv(OBJECT_DOMAIN));
    sponge.absorb_pair(code[0], code[1]);
    sponge.absorb_pair(state, controller[0]);
    sponge.absorb_pair(controller[1], version);
    Address(sponge.finalize_no_pad())
}

/// Eight independently selectable values already present as raw leaves of
/// the canonical body hash.  Old and successor object roots are bound
/// separately through input_owner and output0.owner.
fn contract_context(body: &TxBody) -> [Block128; PROGRAM_STEPS] {
    let leaves = body_hash_leaves(body);
    [
        leaves[TX8X2_LEAF_EPOCH_ANCHOR][0],
        leaves[TX8X2_LEAF_EPOCH_ANCHOR][1],
        leaves[TX8X2_LEAF_FEE][0],
        leaves[TX8X2_LEAF_OUTPUT0_DATA][1],
        leaves[TX8X2_LEAF_OUTPUT1_DATA][1],
        leaves[TX8X2_LEAF_OUTPUT1_OWNER][0],
        leaves[TX8X2_LEAF_OUTPUT1_OWNER][1],
        leaves[TX8X2_LEAF_FLAGS][0],
    ]
}

fn digest_hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").unwrap();
    }
    out
}

fn save_new(path: &str, value: &Value) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .expect("result path must be new");
    serde_json::to_writer_pretty(&mut file, value).unwrap();
    file.write_all(b"\n").unwrap();
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(
        args.len() <= 2,
        "usage: contract_abi_binding [NEW_JSON_FILE]"
    );

    let program: [[Block128; 2]; PROGRAM_STEPS] = std::array::from_fn(|step| {
        [
            Block128::from((step % 4) as u128),
            fixture_field(step, 0x434F_4E54_5241_4354),
        ]
    });
    let code = code_digest(&program);
    let old_state = fixture_field(0, 0x4F4C_4453_5441_5445);
    let next_state = fixture_field(0, 0x4E45_5854_5354_4154);
    let controller = [
        fixture_field(0, 0x434F_4E54_524F_4C4C),
        fixture_field(1, 0x434F_4E54_524F_4C4C),
    ];
    let old_root = object_root(code, old_state, controller, OBJECT_VERSION);
    let next_root = object_root(code, next_state, controller, OBJECT_VERSION);
    assert_ne!(old_root, next_root);

    let recipient = address_from_fields([
        fixture_field(0, 0x5245_4349_5049_454E),
        fixture_field(1, 0x5245_4349_5049_454E),
    ]);
    let mut inputs = [TxInput::dummy(); TX_INPUTS];
    inputs[0] = TxInput {
        slot_index: 5,
        amount: 100,
        creation_id: 7,
    };
    let mut outputs = [TxOutput::dummy(); TX_OUTPUTS];
    outputs[0] = TxOutput {
        slot_index: 8,
        amount: 90,
        owner: next_root,
    };
    outputs[1] = TxOutput {
        slot_index: 9,
        amount: 9,
        owner: recipient,
    };
    let body = TxBody {
        epoch_anchor: [0xA5; 32],
        fee: 1,
        input_owner: old_root,
        inputs,
        outputs,
        validity_bitmap: 1 | output_bitmap_bit(0) | output_bitmap_bit(1),
        is_coinbase: false,
    };
    body.validate_canonical()
        .expect("canonical contract carrier");
    let wire = body.to_bytes();
    assert_eq!(wire.len(), TX_BODY_WIRE_SIZE);
    assert_eq!(TxBody::from_bytes(&wire).unwrap(), body);
    assert_eq!(body.input_owner, old_root);
    assert_eq!(body.outputs[0].owner, next_root);

    // Every one of the sixteen program fields changes both object roots.
    let mut program_field_mutations_bound = 0usize;
    for step in 0..PROGRAM_STEPS {
        for lane in 0..2 {
            let mut changed = program;
            changed[step][lane] += Block128::ONE;
            let changed_code = code_digest(&changed);
            assert_ne!(changed_code, code);
            assert_ne!(
                object_root(changed_code, old_state, controller, OBJECT_VERSION),
                old_root
            );
            assert_ne!(
                object_root(changed_code, next_state, controller, OBJECT_VERSION),
                next_root
            );
            program_field_mutations_bound += 1;
        }
    }

    // Code, mutable State, controller and version are all binding inputs to
    // an object root under the exact three-permutation schedule used by the
    // Meta-A experiment.
    let mut object_field_mutations_bound = 0usize;
    for field in 0..6 {
        let mut changed_code = code;
        let mut changed_state = old_state;
        let mut changed_controller = controller;
        let mut changed_version = OBJECT_VERSION;
        match field {
            0 => changed_code[0] += Block128::ONE,
            1 => changed_code[1] += Block128::ONE,
            2 => changed_state += Block128::ONE,
            3 => changed_controller[0] += Block128::ONE,
            4 => changed_controller[1] += Block128::ONE,
            5 => changed_version += Block128::ONE,
            _ => unreachable!(),
        }
        assert_ne!(
            object_root(
                changed_code,
                changed_state,
                changed_controller,
                changed_version,
            ),
            old_root
        );
        object_field_mutations_bound += 1;
    }

    // For each candidate context lane, alter the exact underlying TxBody
    // field and require both the selected context and canonical Tx hash to
    // change.  Some mutations deliberately cease to be balanced/canonical;
    // this test establishes hash binding, not alternate transaction validity.
    let baseline_context = contract_context(&body);
    let baseline_txid = body.txid();
    let mutations: Vec<(&str, TxBody)> = vec![
        ("epoch_anchor_lo", {
            let mut changed = body.clone();
            changed.epoch_anchor[0] ^= 1;
            changed
        }),
        ("epoch_anchor_hi", {
            let mut changed = body.clone();
            changed.epoch_anchor[16] ^= 1;
            changed
        }),
        ("fee", {
            let mut changed = body.clone();
            changed.fee ^= 1;
            changed
        }),
        ("successor_amount", {
            let mut changed = body.clone();
            changed.outputs[0].amount ^= 1;
            changed
        }),
        ("payment_amount", {
            let mut changed = body.clone();
            changed.outputs[1].amount ^= 1;
            changed
        }),
        ("recipient_lo", {
            let mut changed = body.clone();
            changed.outputs[1].owner.0[0] ^= 1;
            changed
        }),
        ("recipient_hi", {
            let mut changed = body.clone();
            changed.outputs[1].owner.0[16] ^= 1;
            changed
        }),
        ("validity_flags", {
            let mut changed = body.clone();
            changed.validity_bitmap ^= 1 << 1;
            changed
        }),
    ];
    let mut context_bindings = Vec::new();
    for (index, (name, changed)) in mutations.into_iter().enumerate() {
        let changed_context = contract_context(&changed);
        assert_ne!(changed_context[index], baseline_context[index]);
        assert_ne!(changed.txid(), baseline_txid);
        context_bindings.push(json!({
            "program_step": index,
            "source": name,
            "selected_cell_changed": true,
            "txid_changed": true
        }));
    }

    let old_state_leaf = hash_utxo_leaf(
        pack_amount_creation_id(body.inputs[0].amount, body.inputs[0].creation_id).0,
        &old_root,
    );
    let next_state_leaf = body.outputs[0].commitment_with_creation_id(8);
    assert_ne!(old_state_leaf.0, next_state_leaf.0);

    let result = json!({
        "schema": 1,
        "unix_time": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "source_revision": String::from_utf8(
            std::process::Command::new("git").args(["rev-parse", "HEAD"]).output().unwrap().stdout
        ).unwrap().trim(),
        "kind": "contract_object_to_existing_tx8x2_binding",
        "object_commitment": {
            "code_program_steps": PROGRAM_STEPS,
            "code_program_cells": PROGRAM_CELLS,
            "code_opening_bytes": PROGRAM_CELLS * 16,
            "mutable_state_cells": 1,
            "mutable_state_opening_bytes": 16,
            "controller_cells": 2,
            "controller_opening_bytes": 32,
            "protocol_version_cells": 1,
            "holder_receipt_bytes_when_version_is_protocol_constant": PROGRAM_CELLS * 16 + 16 + 32,
            "poseidon_permutations": {
                "code": PROGRAM_STEPS,
                "one_object_root": 3,
                "old_and_successor_roots_total": PROGRAM_STEPS + 6
            },
            "program_field_mutations_tested": program_field_mutations_bound,
            "all_program_field_mutations_change_both_roots": true,
            "object_field_mutations_tested": object_field_mutations_bound,
            "all_object_field_mutations_change_root": true,
            "code_digest_hex": digest_hex(&address_from_fields(code).0),
            "old_object_root_hex": digest_hex(&old_root.0),
            "successor_object_root_hex": digest_hex(&next_root.0)
        },
        "canonical_transaction": {
            "wire_bytes": wire.len(),
            "wire_format_changed": false,
            "canonical_validation_passed": true,
            "wire_roundtrip_passed": true,
            "old_object_root_is_input_owner": body.input_owner == old_root,
            "successor_object_root_is_output0_owner": body.outputs[0].owner == next_root,
            "output1_remains_an_independent_payment": body.outputs[1].owner == recipient,
            "txid_hex": digest_hex(&baseline_txid.0),
            "existing_old_state_leaf_commitment_reused": true,
            "existing_successor_state_leaf_commitment_reused": true
        },
        "context_vector": {
            "cells": PROGRAM_STEPS,
            "bindings": context_bindings,
            "all_selected_cells_are_raw_tx_hash_leaves": true,
            "all_mutations_change_txid": true
        },
        "established": [
            "an immutable eight-step program and one-field mutable object State bind to owner-sized roots with the measured Meta-A schedule",
            "the old object root fits the existing shared input_owner and the successor root fits output0.owner",
            "the carrier is a canonical 323-byte Tx8x2 body and round-trips without a wire-format change",
            "the exact current State-leaf commitment format is reused for both old and successor objects",
            "eight independently selected program contexts come directly from Tx-body hash leaves and every tested source mutation changes the transaction id",
            "the program, object State and controller opening is holder-retained data rather than permanent chain data"
        ],
        "limits": [
            "the current shared input_owner gives one independently controlled mutable object or policy root per logical transaction",
            "this experiment keeps code and controller immutable across a call",
            "the one-field object State and four toy opcodes are not a final contract language or ABI",
            "hash binding alone does not prove useful escrow, recovery, timelock or integer semantics",
            "integrated HistoryStep links, native capsule proof and soundness composition remain to be built"
        ]
    });
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    if let Some(path) = args.get(1) {
        save_new(path, &result);
    }
}
