//! Research-only rank and cell-budget audit for a shortened capsule coin block.
//!
//! This does not alter production constants. It checks the selected affine
//! code directly and records what would remain if the source-opening coin
//! block were shortened from 1024 to the exact 65 * 8 observation ceiling.

use std::collections::{BTreeSet, HashSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use noid_core::{Block128, TowerField};
use noid_fri_binius::zk_affine_code::{
    ZkAffineLchCode, AFFINE_CODE_LEN, AFFINE_CODE_LOG_LEN, AFFINE_CODE_MESSAGE_LEN,
    AFFINE_FRESH_PADDING_START, AFFINE_LIBRA_MASK_START, AFFINE_PCS_COINS_LEN,
    AFFINE_PCS_COINS_START,
};
use noid_fri_binius::zk_capsule_algebra::joint_source_leaf_positions;
use noid_fri_binius::zk_capsule_pcs::ZK_CAPSULE_PCS_QUERY_COUNT;
use noid_gkr::ZK_AUTHORIZATION_MAX_WIRE_BYTES;
use noid_tx::MAX_TX_AUTHORIZATION_BYTES;
use serde_json::{json, Value};

const SOURCE_POSITIONS_PER_LEAF: usize = 8;
const PROPOSED_SOURCE_COINS: usize = ZK_CAPSULE_PCS_QUERY_COUNT * SOURCE_POSITIONS_PER_LEAF;
const TERMINAL_PAD_START: usize = AFFINE_FRESH_PADDING_START;
const TERMINAL_PAD_LEN: usize = 5;
const POSEIDON_STATE_LANES: usize = 4;
const POSEIDON_STORED_ROWS: usize = 67;
const CONTRACT_OBJECT_FIELDS: usize = 4;
const CONTRACT_OBJECT_PERMUTATIONS: usize = CONTRACT_OBJECT_FIELDS / 2;

fn raw_subspace_eval(
    level: usize,
    point: Block128,
    normalizers: &[Block128; AFFINE_CODE_LOG_LEN],
) -> Block128 {
    let mut value = point;
    for previous in 0..level {
        value *= value + normalizers[previous];
    }
    value
}

fn selected_normalizers(code: &ZkAffineLchCode) -> [Block128; AFFINE_CODE_LOG_LEN] {
    let basis = code.basis();
    let mut normalizers = [Block128::ZERO; AFFINE_CODE_LOG_LEN];
    for level in 0..AFFINE_CODE_LOG_LEN {
        normalizers[level] = raw_subspace_eval(level, basis[level], &normalizers);
        assert_ne!(normalizers[level], Block128::ZERO);
    }
    normalizers
}

fn normalized_subspace_eval(
    level: usize,
    point: Block128,
    normalizers: &[Block128; AFFINE_CODE_LOG_LEN],
) -> Block128 {
    raw_subspace_eval(level, point, normalizers) * normalizers[level].invert()
}

fn novel_basis_eval(
    basis_index: usize,
    point: Block128,
    normalizers: &[Block128; AFFINE_CODE_LOG_LEN],
) -> Block128 {
    let mut value = Block128::ONE;
    for level in 0..AFFINE_CODE_LOG_LEN {
        if (basis_index >> level) & 1 == 1 {
            value *= normalized_subspace_eval(level, point, normalizers);
        }
    }
    value
}

fn high_block_matrix(
    code: &ZkAffineLchCode,
    normalizers: &[Block128; AFFINE_CODE_LOG_LEN],
    positions: &[usize],
) -> Vec<Vec<Block128>> {
    positions
        .iter()
        .map(|&position| {
            let point = code.domain_point(position).unwrap();
            let factor = normalized_subspace_eval(10, point, normalizers);
            assert_ne!(factor, Block128::ZERO);
            let low_levels: [Block128; 10] =
                std::array::from_fn(|level| normalized_subspace_eval(level, point, normalizers));
            let mut low = vec![Block128::ZERO; positions.len()];
            low[0] = Block128::ONE;
            for index in 1..positions.len() {
                let bit = index.trailing_zeros() as usize;
                low[index] = low[index ^ (1usize << bit)] * low_levels[bit];
            }
            low.into_iter().map(|value| factor * value).collect()
        })
        .collect()
}

fn field_rank(mut matrix: Vec<Vec<Block128>>) -> usize {
    let rows = matrix.len();
    let columns = matrix.first().map_or(0, Vec::len);
    let mut rank = 0;
    for column in 0..columns {
        let Some(pivot) = (rank..rows).find(|&row| matrix[row][column] != Block128::ZERO) else {
            continue;
        };
        matrix.swap(rank, pivot);
        let inverse = matrix[rank][column].invert();
        for entry in &mut matrix[rank][column..] {
            *entry *= inverse;
        }
        let pivot_row = matrix[rank][column..].to_vec();
        for row in rank + 1..rows {
            let factor = matrix[row][column];
            if factor == Block128::ZERO {
                continue;
            }
            for (entry, &pivot_entry) in matrix[row][column..].iter_mut().zip(&pivot_row) {
                *entry += factor * pivot_entry;
            }
        }
        rank += 1;
        if rank == rows {
            break;
        }
    }
    rank
}

fn positions_for_leaves(leaves: &[usize]) -> Vec<usize> {
    let mut positions = BTreeSet::new();
    for &leaf in leaves {
        positions.extend(joint_source_leaf_positions(leaf).unwrap());
    }
    positions.into_iter().collect()
}

fn rank_case(
    name: &str,
    code: &ZkAffineLchCode,
    normalizers: &[Block128; AFFINE_CODE_LOG_LEN],
    leaves: &[usize],
) -> Value {
    let positions = positions_for_leaves(leaves);
    let started = Instant::now();
    let rank = field_rank(high_block_matrix(code, normalizers, &positions));
    let elapsed_ms = started.elapsed().as_millis();
    assert_eq!(rank, positions.len());
    json!({
        "name": name,
        "submitted_leaves": leaves.len(),
        "distinct_leaves": leaves.iter().copied().collect::<BTreeSet<_>>().len(),
        "distinct_positions": positions.len(),
        "matrix_rows": positions.len(),
        "matrix_columns": positions.len(),
        "rank": rank,
        "elapsed_ms": elapsed_ms,
    })
}

fn source_revision() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn main() {
    assert_eq!(ZK_CAPSULE_PCS_QUERY_COUNT, 65);
    assert_eq!(PROPOSED_SOURCE_COINS, 520);
    assert_eq!(AFFINE_PCS_COINS_START, 1_024);
    assert_eq!(AFFINE_PCS_COINS_LEN, 1_024);

    let code = ZkAffineLchCode::selected().unwrap();
    let normalizers = selected_normalizers(&code);

    let distinct_domain_points = (0..AFFINE_CODE_LEN)
        .map(|position| code.domain_point(position).unwrap().to_u128())
        .collect::<HashSet<_>>()
        .len();
    assert_eq!(distinct_domain_points, AFFINE_CODE_LEN);

    let nonzero_w10_positions = (0..AFFINE_CODE_LEN)
        .filter(|&position| {
            normalized_subspace_eval(10, code.domain_point(position).unwrap(), &normalizers)
                != Block128::ZERO
        })
        .count();
    assert_eq!(nonzero_w10_positions, AFFINE_CODE_LEN);

    // Tie the independent evaluator to the selected production encoder at
    // the beginning, middle, and end of the proposed coefficient block.
    let encoder_basis_indices = [1_024usize, 1_025, 1_281, 1_543];
    let mut encoder_checks = Vec::new();
    for basis_index in encoder_basis_indices {
        let mut message = vec![Block128::ZERO; AFFINE_CODE_MESSAGE_LEN];
        message[basis_index] = Block128::ONE;
        let encoded = code.encode(&message).unwrap();
        let matches = (0..AFFINE_CODE_LEN)
            .filter(|&position| {
                encoded[position]
                    == novel_basis_eval(
                        basis_index,
                        code.domain_point(position).unwrap(),
                        &normalizers,
                    )
            })
            .count();
        assert_eq!(matches, AFFINE_CODE_LEN);
        encoder_checks.push(json!({
            "basis_index": basis_index,
            "positions_checked": AFFINE_CODE_LEN,
            "matches": matches,
        }));
    }

    let contiguous_leaves: Vec<_> = (0..ZK_CAPSULE_PCS_QUERY_COUNT).collect();
    let strided_leaves: Vec<_> = (0..ZK_CAPSULE_PCS_QUERY_COUNT)
        .map(|index| (index * 127 + 19) & ((AFFINE_CODE_LEN / SOURCE_POSITIONS_PER_LEAF) - 1))
        .collect();
    let edge_leaves: Vec<_> = (0..ZK_CAPSULE_PCS_QUERY_COUNT)
        .map(|index| {
            if index % 2 == 0 {
                index / 2
            } else {
                AFFINE_CODE_LEN / SOURCE_POSITIONS_PER_LEAF - 1 - index / 2
            }
        })
        .collect();
    let repeated_leaves = vec![4_097usize; ZK_CAPSULE_PCS_QUERY_COUNT];
    let rank_cases = vec![
        rank_case("contiguous-max", &code, &normalizers, &contiguous_leaves),
        rank_case("strided-max", &code, &normalizers, &strided_leaves),
        rank_case("edge-max", &code, &normalizers, &edge_leaves),
        rank_case("one-leaf-repeated", &code, &normalizers, &repeated_leaves),
    ];

    let current_source_coin_end = AFFINE_PCS_COINS_START + AFFINE_PCS_COINS_LEN;
    let proposed_source_coin_end = AFFINE_PCS_COINS_START + PROPOSED_SOURCE_COINS;
    let reclaimed_source_tail = current_source_coin_end - proposed_source_coin_end;
    let free_middle_cells = AFFINE_PCS_COINS_START - (TERMINAL_PAD_START + TERMINAL_PAD_LEN);
    let candidate_trace_cells = AFFINE_LIBRA_MASK_START + free_middle_cells + reclaimed_source_tail;

    // Conservative storage: each standard Poseidon trace retains all 67
    // four-lane rows. A four-field object commitment takes two permutations;
    // old and new commitments therefore take four traces total.
    let cells_per_object_commitment =
        CONTRACT_OBJECT_PERMUTATIONS * POSEIDON_STORED_ROWS * POSEIDON_STATE_LANES;
    let old_and_new_commitment_cells = 2 * cells_per_object_commitment;
    let remaining_after_commitments = candidate_trace_cells - old_and_new_commitment_cells;
    let two_capsule_worst_bytes = 2 * ZK_AUTHORIZATION_MAX_WIRE_BYTES;

    let result = json!({
        "kind": "v2-capsule-bank-budget",
        "status": "source-rank-established-full-zk-not-established",
        "source_revision": source_revision(),
        "generated_unix_seconds": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "selected_code": {
            "message_len": AFFINE_CODE_MESSAGE_LEN,
            "domain_len": AFFINE_CODE_LEN,
            "domain_distinct_points": distinct_domain_points,
            "w10_nonzero_positions": nonzero_w10_positions,
        },
        "source_observation_ceiling": {
            "query_leaves": ZK_CAPSULE_PCS_QUERY_COUNT,
            "positions_per_leaf": SOURCE_POSITIONS_PER_LEAF,
            "maximum_distinct_positions": PROPOSED_SOURCE_COINS,
            "current_independent_coins": AFFINE_PCS_COINS_LEN,
            "proposed_independent_coins": PROPOSED_SOURCE_COINS,
            "reclaimed_tail_cells": reclaimed_source_tail,
            "proposed_coin_range": [AFFINE_PCS_COINS_START, proposed_source_coin_end],
            "reclaimed_range": [proposed_source_coin_end, current_source_coin_end],
        },
        "encoder_basis_checks": encoder_checks,
        "rank_cases": rank_cases,
        "candidate_bank_layout": {
            "state_slice_cells": AFFINE_LIBRA_MASK_START,
            "libra_reserved_range": [AFFINE_LIBRA_MASK_START, AFFINE_FRESH_PADDING_START],
            "terminal_pad_reserved_range": [TERMINAL_PAD_START, TERMINAL_PAD_START + TERMINAL_PAD_LEN],
            "free_middle_cells": free_middle_cells,
            "reclaimed_source_tail_cells": reclaimed_source_tail,
            "total_candidate_trace_cells": candidate_trace_cells,
        },
        "conservative_two_capsule_candidate": {
            "contract_object_fields": CONTRACT_OBJECT_FIELDS,
            "permutations_per_object_commitment": CONTRACT_OBJECT_PERMUTATIONS,
            "stored_cells_per_permutation": POSEIDON_STORED_ROWS * POSEIDON_STATE_LANES,
            "old_and_new_commitment_cells": old_and_new_commitment_cells,
            "remaining_trace_cells": remaining_after_commitments,
            "wallet_capsule_worst_bytes": ZK_AUTHORIZATION_MAX_WIRE_BYTES,
            "transition_capsule_worst_bytes_if_shape_unchanged": ZK_AUTHORIZATION_MAX_WIRE_BYTES,
            "two_capsule_worst_bytes_before_bundle_framing": two_capsule_worst_bytes,
            "current_authorization_payload_limit": MAX_TX_AUTHORIZATION_BYTES,
            "fits_current_payload_limit_before_bundle_framing": two_capsule_worst_bytes < MAX_TX_AUTHORIZATION_BYTES,
        },
        "established": [
            "The selected affine domain contains 65536 distinct points.",
            "The W_10 factor is nonzero at every selected domain point.",
            "The first 520 high novel-basis coefficients have full row rank for every q <= 520 distinct source positions by the novel-basis/Vandermonde factorization.",
            "Representative 520-by-520 matrices built from the selected code have rank 520.",
            "Shortening only the source coin block to 520 would reclaim 504 committed bank cells without changing the 2048-symbol code geometry.",
            "Two unchanged-shape worst-case capsule payloads fit below the current 256 KiB authorization limit before small bundle framing.",
        ],
        "not_established": [
            "The complete joint hiding ledger after replacing fresh suffix cells with witness trace cells.",
            "A simulator argument after conditioning on every transcript field and terminal relation.",
            "A sound fragmented-bank transition relation.",
            "Program authentication, persistent object encoding, public effect binding, or replay rules.",
            "An integrated HistoryStep row count, proof time, verification time, or propagation measurement.",
            "That two capsules can be represented by the current one-capsule-per-authorization-group outer layout.",
        ],
    });

    let encoded = serde_json::to_vec_pretty(&result).unwrap();
    if let Some(path) = std::env::args().nth(1) {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("output path must be new");
        file.write_all(&encoded).unwrap();
        file.write_all(b"\n").unwrap();
    } else {
        std::io::stdout().write_all(&encoded).unwrap();
        std::io::stdout().write_all(b"\n").unwrap();
    }
}
