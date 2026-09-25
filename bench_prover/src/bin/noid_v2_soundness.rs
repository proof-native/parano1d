// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Read-only exact accounting for an explicitly pinned joint bank and keys.
//! No matrix generation, node configuration or release activation is performed.

use noid_ivc_core::matrix_claim::sparse_c1::{
    SparseMatrixEvaluationKey, SPARSE_EVALUATION_KEY_BYTES,
};
use noid_miner::{history_step_artifacts as old, v2_artifacts as next};
use noid_recursive::acceptance::history_step::v2::banked::{Class, PinnedRetirementKeys};
use noid_soundness::{exact::ExactProbability, parameters::HistoryClassParameters};
use serde_json::{json, Value};
use std::{io::Read, path::Path};

fn pin(value: &str) -> Result<[u8; 32], String> {
    hex::decode(value)
        .map_err(|e| e.to_string())?
        .try_into()
        .map_err(|_| "32-byte pin required".into())
}

fn read(path: &str, max: usize) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(Path::new(path)).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > max as u64 {
        return Err("artifact type or size bound".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > max {
        return Err("artifact grew beyond its bound".into());
    }
    Ok(bytes)
}

fn probability(value: &ExactProbability) -> Value {
    json!({"exact":value.exact_fraction(),"upper_decimal":value.decimal_ceiling(90),
        "security_bits_interval":value.security_bits().to_string()})
}

fn geometry(value: &HistoryClassParameters) -> Value {
    json!({"label":value.tier,"message_log2":value.message_log2,
        "codeword_log2":value.codeword_log2,"inverse_rate":value.inverse_rate,
        "plaintext_tail_len":value.plaintext_tail_len,"fri_arities":value.fri_arities})
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 8 {
        return Err("usage: noid_v2_soundness LEGACY_METADATA LEGACY_PIN V2_METADATA BANK_PIN KEY_0 KEY_0_PIN KEY_1 KEY_1_PIN".into());
    }
    let legacy = old::decode_history_step_runtime_metadata_pinned(
        &read(&args[0], old::HISTORY_STEP_RUNTIME_METADATA_MAX_BYTES)?,
        pin(&args[1])?,
    )
    .map_err(|e| e.to_string())?;
    let metadata = next::decode_v2_runtime_metadata_pinned(
        &read(&args[2], next::V2_METADATA_MAX_BYTES)?,
        pin(&args[3])?,
    )?;
    let encoded = [
        read(&args[4], SPARSE_EVALUATION_KEY_BYTES)?,
        read(&args[6], SPARSE_EVALUATION_KEY_BYTES)?,
    ];
    let pins = [pin(&args[5])?, pin(&args[7])?];
    PinnedRetirementKeys::from_release(legacy.bank(), [&encoded[0], &encoded[1]], pins)
        .map_err(|e| e.to_string())?;
    let keys = [
        SparseMatrixEvaluationKey::from_bytes_pinned(&encoded[0], pins[0]),
        SparseMatrixEvaluationKey::from_bytes_pinned(&encoded[1], pins[1]),
    ]
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;
    let keys: [SparseMatrixEvaluationKey; 2] = keys.try_into().map_err(|_| "key count")?;
    let result = noid_soundness::v2::calculate(metadata.parts(), &keys)?;
    let category = &result.category_one;
    let bank = metadata.bank();
    let classes = Class::ALL.map(|class| {
        let config = bank.config().class(class);
        json!({"class":class.wire_id(),"matrix":hex::encode(bank.matrix_digest(class)),
            "m":config.outer_m(),"pages":config.pages(),"inputs":config.max_live_inputs(),
            "calls":config.contract_slots(),"pcs":geometry(&result.joint_classes[class.index()])})
    });
    let retirement: Vec<_> = result
        .retirement
        .iter()
        .map(|proof| {
            json!({
                "key":hex::encode(proof.key_digest),"matrix":hex::encode(proof.matrix_digest),
                "columns":proof.columns.iter().map(geometry).collect::<Vec<_>>(),
                "dynamic_commitments":proof.dynamic_commitments,
                "multiplicity":noid_soundness::v2::RETIREMENT_MULTIPLICITY,
                "maximum_initial_list_size":proof.maximum_initial_list_size.to_string(),
                "candidate_tuples":proof.candidate_tuples.to_string(),
                "memory_polynomial_roots":proof.memory_polynomial_roots,
                "query_escape":probability(&proof.query_escape),
                "maximum_proximity_exception":probability(&proof.maximum_proximity_exception),
                "pcs_scalar_exception":probability(&proof.pcs_scalar_exception),
                "reduction_exception":probability(&proof.reduction_exception),
                "local_rbr":probability(&proof.local_rbr),
                "minimum_query_permutations":proof.minimum_query_permutations,
            })
        })
        .collect();
    let report = json!({"status":"conditional source-linked v2 soundness accounting",
        "premises":"noid_soundness/docs/v2-retirement.md; existing all-root, fixed-Poseidon2b and resource-price premises",
        "bank":hex::encode(bank.digest()),"legacy_runtime":args[1],
        "activation_height":bank.config().activation_height(),"classes":classes,"retirement":retirement,
        "sequential_local_rbr":probability(&result.sequential_local_rbr),
        "sequential_largest_query_cap":result.sequential_largest_query_cap.to_string(),
        "sequential_at_two_to_64":probability(&result.sequential_at_two_to_64.total),
        "resource_limiting_event":category.limiting_event,
        "resource_work_floor_exact":category.dominant_half_success_gate_depth_floor.exact_fraction(),
        "resource_work_floor_bits":category.dominant_half_success_gate_depth_floor.descriptive_bits(),
        "category_one_ideal_envelope":probability(&category.ideal_envelope),
        "fixed_poseidon2b_delta_headroom":probability(&category.fixed_poseidon2b_delta_headroom),
        "events":category.events.iter().map(|event| json!({"id":event.id,
            "bad_density":probability(&event.bad_density),
            "response_gate_depth_price":event.response_cost.gate_depth_product().to_string(),
            "density_per_gate_depth":probability(&event.bad_density_per_gate_depth)})).collect::<Vec<_>>(),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("v2 soundness accounting failed: {error}");
        std::process::exit(1);
    }
}
