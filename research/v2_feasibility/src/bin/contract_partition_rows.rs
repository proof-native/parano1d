//! Research-only row count for a stable B25 split between contract and
//! ordinary wallet authorization tiles.
//!
//! The proposed fixed layout reserves authorization tiles 0..4 for contract
//! proofs and 4..32 for the current wallet proof.  Live body pages are
//! canonically ordered as a prefix of at most four contract pages followed by
//! ordinary pages.  This harness measures the exact base-field R1CS cost of
//! shifting the existing four-field wallet statements past that contract
//! prefix.  It does not modify the production transaction format.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use noid_ivc_core::field::F128;
use noid_recursive::acceptance::trace::{mul, pin_zero, FieldR1csBuilder, LinExpr};
use serde_json::{json, Value};

const BODY_PAGES: usize = 25;
const CONTRACT_PREFIX_MAX: usize = 4;
const STATEMENT_FIELDS: usize = 4;
const AUTH_TILES: usize = 32;
const CONTRACT_TILES: usize = 4;
const WALLET_TILES: usize = AUTH_TILES - CONTRACT_TILES;

const B25_USEFUL_ROWS: usize = 4_185_273;
const B25_PADDED_ROWS: usize = 1 << 22;
const UNIVERSAL_CONTRACT_ROWS_PER_TILE: usize = 402;

fn value(page: usize, field: usize) -> F128 {
    F128 {
        lo: 0xC011_0000_0000_0000u64 ^ ((page as u64) << 8) ^ field as u64,
        hi: 0xA11A_5000_0000_0000u64 ^ ((field as u64) << 16) ^ page as u64,
    }
}

fn allocate_body_statements(b: &mut FieldR1csBuilder) -> [[LinExpr; STATEMENT_FIELDS]; BODY_PAGES] {
    std::array::from_fn(|page| {
        std::array::from_fn(|field| LinExpr::from_wire(b.alloc_f128(value(page, field))))
    })
}

/// Select `body[k + contract_count]` without a one-hot decoder.  For a
/// monotone prefix t = 1110, each conditional advances the current candidate
/// once.  Candidates beyond B25 are the canonical ghost statement, modelled
/// as zero here because adding protocol constants is linear and costs no row.
fn shifted_wallet_statement(
    b: &mut FieldR1csBuilder,
    body: &[[LinExpr; STATEMENT_FIELDS]; BODY_PAGES],
    contract_prefix: &[LinExpr; CONTRACT_PREFIX_MAX],
    wallet_index: usize,
) -> [LinExpr; STATEMENT_FIELDS] {
    std::array::from_fn(|field| {
        let mut selected = body
            .get(wallet_index)
            .map(|statement| statement[field].clone())
            .unwrap_or_else(LinExpr::zero);
        for (step, contract) in contract_prefix.iter().enumerate() {
            let candidate = body
                .get(wallet_index + step + 1)
                .map(|statement| statement[field].clone())
                .unwrap_or_else(LinExpr::zero);
            let delta = selected.add(&candidate);
            selected = selected.add(&mul(b, contract, &delta));
        }
        selected
    })
}

fn run_case(contract_count: usize) -> Value {
    assert!(contract_count <= CONTRACT_PREFIX_MAX);
    let mut b = FieldR1csBuilder::new();
    let body = allocate_body_statements(&mut b);
    let baseline_after_existing_aliases = b.num_wires();

    // In production these four values can be aliases of already boolean body
    // type bits.  Allocate booleans here so the gross result also covers a
    // design which needs fresh type-bit constraints.
    let contract_prefix: [LinExpr; CONTRACT_PREFIX_MAX] =
        std::array::from_fn(|index| LinExpr::from_wire(b.alloc_bool(index < contract_count)));
    let after_fresh_boolean_bits = b.num_wires();

    // Enforce the canonical 111..000 prefix.  `t[i+1] * (1+t[i]) = 0`.
    for index in 0..CONTRACT_PREFIX_MAX - 1 {
        let violation = mul(
            &mut b,
            &contract_prefix[index + 1],
            &contract_prefix[index].add_const(F128::ONE),
        );
        pin_zero(&mut b, &violation);
    }
    let after_prefix_checks = b.num_wires();

    let selected: [[LinExpr; STATEMENT_FIELDS]; BODY_PAGES] = std::array::from_fn(|wallet| {
        shifted_wallet_statement(&mut b, &body, &contract_prefix, wallet)
    });
    let after_selection = b.num_wires();

    for wallet in 0..BODY_PAGES {
        let expected_page = wallet + contract_count;
        for field in 0..STATEMENT_FIELDS {
            let expected = if expected_page < BODY_PAGES {
                value(expected_page, field)
            } else {
                F128::ZERO
            };
            assert_eq!(selected[wallet][field].eval(b.values()), expected);
        }
    }

    let (r1cs, witness) = b.build();
    assert!(r1cs.satisfies(&witness));

    let fresh_boolean_rows = after_fresh_boolean_bits - baseline_after_existing_aliases;
    let prefix_rows = after_prefix_checks - after_fresh_boolean_bits;
    let selection_rows = after_selection - after_prefix_checks;
    let gross_rows = after_selection - baseline_after_existing_aliases;
    let rows_if_existing_boolean_aliases = gross_rows - fresh_boolean_rows;

    json!({
        "contract_count": contract_count,
        "body_pages": BODY_PAGES,
        "contract_tiles": CONTRACT_TILES,
        "wallet_tiles": WALLET_TILES,
        "statement_fields": STATEMENT_FIELDS,
        "fresh_boolean_rows": fresh_boolean_rows,
        "prefix_rows": prefix_rows,
        "selection_rows": selection_rows,
        "gross_incremental_rows": gross_rows,
        "incremental_rows_if_type_bits_reuse_existing_boolean_aliases": rows_if_existing_boolean_aliases,
        "selected_values_match_native_shift": true,
        "r1cs_satisfied": true
    })
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
        "usage: contract_partition_rows [NEW_JSON_FILE]"
    );
    let cases: Vec<_> = (0..=CONTRACT_PREFIX_MAX).map(run_case).collect();
    let gross_rows = cases[0]["gross_incremental_rows"].as_u64().unwrap() as usize;
    assert!(cases
        .iter()
        .all(|case| { case["gross_incremental_rows"].as_u64().unwrap() as usize == gross_rows }));
    let projected =
        B25_USEFUL_ROWS + CONTRACT_TILES * UNIVERSAL_CONTRACT_ROWS_PER_TILE + gross_rows;
    let result = json!({
        "schema": 1,
        "unix_time": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "source_revision": String::from_utf8(
            std::process::Command::new("git").args(["rev-parse", "HEAD"]).output().unwrap().stdout
        ).unwrap().trim(),
        "kind": "b25_fixed_contract_wallet_tile_partition_recursive_row_model",
        "layout": {
            "contract_authorization_tiles": "0..4",
            "wallet_authorization_tiles": "4..32",
            "body_order": "zero to four contract pages, then ordinary wallet pages, then ghosts",
            "ordinary_wallet_page_capacity_preserved": BODY_PAGES,
            "contract_page_capacity": CONTRACT_PREFIX_MAX
        },
        "cases": cases,
        "projection": {
            "current_b25_useful_rows": B25_USEFUL_ROWS,
            "four_universal_contract_tiles_rows": CONTRACT_TILES * UNIVERSAL_CONTRACT_ROWS_PER_TILE,
            "partition_rows_gross": gross_rows,
            "projected_useful_rows_before_object_hash_and_history_links": projected,
            "rows_left_in_m22_before_object_hash_and_history_links": B25_PADDED_ROWS - projected,
            "stays_m22": projected <= B25_PADDED_ROWS
        },
        "established": [
            "a fixed four-contract and twenty-eight-wallet authorization layout preserves the full twenty-five-page ordinary B25 capacity",
            "all five possible contract counts use one identical R1CS shape",
            "a four-stage conditional shift maps each ordinary statement past the contract prefix",
            "the generated R1CS witnesses satisfy the matrix and every selected value matches the native index shift"
        ],
        "not_established": [
            "production body type-bit allocation and canonical transaction ordering",
            "heterogeneous capsule verification in the selected all-tiles relation",
            "contract statement fields, object hashes, effect binding, or complete HistoryStep integration"
        ]
    });
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    if let Some(path) = args.get(1) {
        save_new(path, &result);
    }
}
