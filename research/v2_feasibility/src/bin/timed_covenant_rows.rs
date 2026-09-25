//! Research-only recursive row model for a useful two-branch timed covenant.
//!
//! The claimant proves one ordinary Parano1d spend secret in the reused
//! authorization capsule.  Before the object deadline, one committed
//! claimant and recipient pair is accepted.  At or after the deadline, a
//! second committed claimant and recipient pair is accepted.  This directly
//! covers hashlock/refund escrow and delayed recovery without requiring the
//! authorization proof to know the exact inclusion height.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use noid_core::Block128;
use noid_recursive::acceptance::trace::{
    alloc_block, const_block, lt_strict_expr, mul, pin_eq, range_check_bits, FieldR1csBuilder,
    LinExpr, F128,
};
use serde_json::{json, Value};

const B25_META_CONTRACT_GHOST_USEFUL_ROWS: usize = 4_185_327;
const B25_PADDED_ROWS: usize = 1 << 22;
const CONTRACT_TILES: usize = 4;
const CONTRACT_CAPSULE_RECURSIVE_ROWS_PER_TILE: usize = 465;
const FIXED_PARTITION_ROWS: usize = 410;
const CONSERVATIVE_META_AND_HISTORY_EQUALITY_LINKS: usize = 160;
const TERMINAL_CONTRACT_BITMAP: u16 = 1 | (1 << 8);

#[derive(Clone, Copy)]
struct TimedCovenantCase {
    name: &'static str,
    height: u64,
    deadline: u64,
    claim_authority: [Block128; 2],
    refund_authority: [Block128; 2],
    claim_recipient: [Block128; 2],
    refund_recipient: [Block128; 2],
    caller: [Block128; 2],
    recipient: [Block128; 2],
    validity_bitmap: u16,
}

struct BuiltCase {
    name: &'static str,
    rows_after_reused_height_bits: usize,
    deadline_range_rows: usize,
    comparison_rows: usize,
    selection_and_pin_rows: usize,
    matrix_digest: [u8; 32],
    satisfied: bool,
}

fn pair(seed: u128) -> [Block128; 2] {
    [
        Block128::from(seed),
        Block128::from(seed.rotate_left(53) ^ 0x9E37_79B9_7F4A_7C15),
    ]
}

fn select(
    b: &mut FieldR1csBuilder,
    when_before: &LinExpr,
    when_refund: &LinExpr,
    refund_branch: &LinExpr,
) -> LinExpr {
    when_before.add(&mul(b, refund_branch, &when_before.add(when_refund)))
}

fn build_case(case: TimedCovenantCase) -> BuiltCase {
    let mut b = FieldR1csBuilder::new();

    // All of these are aliases to cells already allocated by the accepted
    // accumulator, authorization verifier, contract program boundary and
    // Tx-body spine in an integrated HistoryStep.
    let height = alloc_block(&mut b, Block128::from(case.height as u128));
    let deadline = alloc_block(&mut b, Block128::from(case.deadline as u128));
    let claim_authority = case.claim_authority.map(|value| alloc_block(&mut b, value));
    let refund_authority = case
        .refund_authority
        .map(|value| alloc_block(&mut b, value));
    let claim_recipient = case.claim_recipient.map(|value| alloc_block(&mut b, value));
    let refund_recipient = case
        .refund_recipient
        .map(|value| alloc_block(&mut b, value));
    let caller = case.caller.map(|value| alloc_block(&mut b, value));
    let recipient = case.recipient.map(|value| alloc_block(&mut b, value));
    let validity_bitmap = alloc_block(&mut b, Block128::from(case.validity_bitmap as u128));

    // The accepted accumulator already range-checks block height.  Build it
    // before the measured delta to model exact alias reuse.
    let height_bits = range_check_bits(&mut b, &height, 64);
    let semantic_start = b.num_wires();

    let deadline_start = b.num_wires();
    let deadline_bits = range_check_bits(&mut b, &deadline, 64);
    let deadline_range_rows = b.num_wires() - deadline_start;

    let comparison_start = b.num_wires();
    let before_deadline = lt_strict_expr(&mut b, &height_bits, &deadline_bits);
    let refund_branch = before_deadline.add_const(F128::ONE);
    let comparison_rows = b.num_wires() - comparison_start;

    let selection_start = b.num_wires();
    for lane in 0..2 {
        let expected_caller = select(
            &mut b,
            &claim_authority[lane],
            &refund_authority[lane],
            &refund_branch,
        );
        pin_eq(&mut b, &caller[lane], &expected_caller);
        let expected_recipient = select(
            &mut b,
            &claim_recipient[lane],
            &refund_recipient[lane],
            &refund_branch,
        );
        pin_eq(&mut b, &recipient[lane], &expected_recipient);
    }
    pin_eq(
        &mut b,
        &validity_bitmap,
        &const_block(Block128::from(TERMINAL_CONTRACT_BITMAP as u128)),
    );
    let selection_and_pin_rows = b.num_wires() - selection_start;
    let rows_after_reused_height_bits = b.num_wires() - semantic_start;

    let (r1cs, witness) = b.build();
    BuiltCase {
        name: case.name,
        rows_after_reused_height_bits,
        deadline_range_rows,
        comparison_rows,
        selection_and_pin_rows,
        matrix_digest: r1cs.statement_digest(),
        satisfied: r1cs.satisfies(&witness),
    }
}

fn digest_hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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
        "usage: timed_covenant_rows [NEW_JSON_FILE]"
    );

    let claim_authority = pair(0xC1A1_0001);
    let refund_authority = pair(0xBACC_0002);
    let claim_recipient = pair(0x5E11_0003);
    let refund_recipient = pair(0xBACC_0004);
    let valid_cases = [
        TimedCovenantCase {
            name: "claim_one_block_before_deadline",
            height: 999,
            deadline: 1_000,
            claim_authority,
            refund_authority,
            claim_recipient,
            refund_recipient,
            caller: claim_authority,
            recipient: claim_recipient,
            validity_bitmap: TERMINAL_CONTRACT_BITMAP,
        },
        TimedCovenantCase {
            name: "refund_exactly_at_deadline",
            height: 1_000,
            deadline: 1_000,
            claim_authority,
            refund_authority,
            claim_recipient,
            refund_recipient,
            caller: refund_authority,
            recipient: refund_recipient,
            validity_bitmap: TERMINAL_CONTRACT_BITMAP,
        },
        TimedCovenantCase {
            name: "refund_after_deadline",
            height: 1_001,
            deadline: 1_000,
            claim_authority,
            refund_authority,
            claim_recipient,
            refund_recipient,
            caller: refund_authority,
            recipient: refund_recipient,
            validity_bitmap: TERMINAL_CONTRACT_BITMAP,
        },
    ];
    let built_valid = valid_cases.map(build_case);
    assert!(built_valid.iter().all(|case| case.satisfied));
    assert!(built_valid
        .windows(2)
        .all(|pair| pair[0].matrix_digest == pair[1].matrix_digest));

    let invalid_cases = [
        TimedCovenantCase {
            name: "refund_authority_before_deadline",
            caller: refund_authority,
            ..valid_cases[0]
        },
        TimedCovenantCase {
            name: "claim_authority_at_deadline",
            caller: claim_authority,
            ..valid_cases[1]
        },
        TimedCovenantCase {
            name: "wrong_claim_recipient",
            recipient: refund_recipient,
            ..valid_cases[0]
        },
        TimedCovenantCase {
            name: "wrong_refund_recipient",
            recipient: claim_recipient,
            ..valid_cases[2]
        },
        TimedCovenantCase {
            name: "non_terminal_body_shape",
            validity_bitmap: TERMINAL_CONTRACT_BITMAP | (1 << 9),
            ..valid_cases[2]
        },
    ];
    let built_invalid = invalid_cases.map(build_case);
    assert!(built_invalid.iter().all(|case| !case.satisfied));
    assert!(built_invalid
        .iter()
        .all(|case| case.matrix_digest == built_valid[0].matrix_digest));

    let measured = &built_valid[0];
    let four_semantics_rows = CONTRACT_TILES * measured.rows_after_reused_height_bits;
    let projected_useful_rows = B25_META_CONTRACT_GHOST_USEFUL_ROWS
        + FIXED_PARTITION_ROWS
        + CONTRACT_TILES * CONTRACT_CAPSULE_RECURSIVE_ROWS_PER_TILE
        + four_semantics_rows
        + CONSERVATIVE_META_AND_HISTORY_EQUALITY_LINKS;
    let result = json!({
        "schema": 1,
        "unix_time": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "source_revision": String::from_utf8(
            std::process::Command::new("git").args(["rev-parse", "HEAD"]).output().unwrap().stdout
        ).unwrap().trim(),
        "kind": "timed_two_branch_covenant_recursive_row_model",
        "semantics": {
            "before_deadline": "claim authority must prove its secret and output0 must pay the committed claim recipient",
            "at_or_after_deadline": "refund authority must prove its secret and output0 must pay the committed refund recipient",
            "proof_knows_exact_inclusion_height": false,
            "height_source": "accepted HistoryStep accumulator",
            "deadline_source": "old object State or an authenticated program operand",
            "terminal_body_shape": "one live input and output0 only",
            "covers": ["hashlock/refund escrow", "delayed key recovery", "inheritance or dead-man switch"]
        },
        "measured_increment_per_contract": {
            "deadline_u64_range_rows": measured.deadline_range_rows,
            "height_vs_deadline_comparison_rows": measured.comparison_rows,
            "four_address_lane_selections_and_pins_plus_body_shape": measured.selection_and_pin_rows,
            "total_rows_after_reusing_existing_height_bits": measured.rows_after_reused_height_bits,
            "matrix_digest_hex": digest_hex(&measured.matrix_digest)
        },
        "valid_cases": built_valid.iter().map(|case| json!({
            "name": case.name,
            "r1cs_satisfied": case.satisfied
        })).collect::<Vec<_>>(),
        "invalid_cases": built_invalid.iter().map(|case| json!({
            "name": case.name,
            "rejected_by_unsatisfied_r1cs": !case.satisfied
        })).collect::<Vec<_>>(),
        "same_matrix_for_all_valid_and_invalid_cases": true,
        "b25_conservative_projection": {
            "exact_full_b25_meta_contract_ghost_useful_rows": B25_META_CONTRACT_GHOST_USEFUL_ROWS,
            "fixed_partition_rows": FIXED_PARTITION_ROWS,
            "four_contract_capsule_recursive_rows": CONTRACT_TILES * CONTRACT_CAPSULE_RECURSIVE_ROWS_PER_TILE,
            "four_timed_covenant_semantics_rows": four_semantics_rows,
            "conservative_meta_and_history_equality_link_ceiling": CONSERVATIVE_META_AND_HISTORY_EQUALITY_LINKS,
            "projected_useful_rows": projected_useful_rows,
            "padded_rows": B25_PADDED_ROWS,
            "remaining_rows": B25_PADDED_ROWS - projected_useful_rows,
            "stays_m22": projected_useful_rows <= B25_PADDED_ROWS
        },
        "established": [
            "a useful two-party timed covenant has one content-invariant R1CS shape across both boundary branches",
            "the exact block height can remain an outer HistoryStep input, so a holder does not need to regenerate its private authorization proof for every candidate block",
            "claimant and recipient are both selected by the authenticated deadline branch",
            "the deadline edge is exact: claim is valid at deadline minus one and refund is valid at the deadline",
            "wrong authority, wrong recipient and wrong output shape are rejected",
            "four contract slots remain inside the current B25 m22 envelope under the stated conservative row projection"
        ],
        "not_established": [
            "the same semantics integrated into the production HistoryStep matrix",
            "a universal instruction encoding for this predicate rather than a fixed covenant primitive",
            "continuing objects, multiple independently controlled inputs, calls between objects or storage trees",
            "complete soundness composition and production proving time"
        ]
    });
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    if let Some(path) = args.get(1) {
        save_new(path, &result);
    }
}
