//! Research-only timing of contract-object Poseidon slots appended inside the
//! existing dyadic block-spine envelope.
//!
//! This measures the production block-spine main and shift sumchecks over the
//! exact current and augmented slot counts. It does not implement the mixed
//! Tx8x2/object descriptor, output pins, PCS opening, or HistoryStep recursion.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use noid_core::{Block128, TowerField};
use noid_gkr::block_spine::{
    num_vars_for, prove_block_spine_shift, prove_block_spine_unified, verify_block_spine_shift,
    verify_block_spine_unified,
};
use noid_gkr::{BlockSpineMle, BlockSpineUnifiedReduction};
use noid_poseidon2b::channel::Poseidon2bChannel;
use serde_json::{json, Value};

const TX_SPINE_SLOTS: usize = 31;
const DEFAULT_OBJECT_PERMUTATIONS_PER_CALL: usize = 4;
const B25_TIER: usize = 25;
const B255_TIER: usize = 255;

fn deterministic_field(index: usize, domain: u128) -> Block128 {
    let value = domain
        .wrapping_mul(index as u128 + 1)
        .rotate_left(((29 * index + 7) % 127) as u32)
        ^ (index as u128 + 31).wrapping_mul(0xD6E8_FEB8_6659_FD93);
    let value = Block128::from(value);
    if value == Block128::ZERO || value == Block128::ONE {
        value + Block128::from(2u128)
    } else {
        value
    }
}

fn slot_inputs(count: usize, domain: u128) -> Vec<[Block128; 4]> {
    (0..count)
        .map(|slot| std::array::from_fn(|lane| deterministic_field(4 * slot + lane, domain)))
        .collect()
}

fn median(values: &[u128]) -> u128 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

fn stats(values: &[u128]) -> Value {
    json!({
        "samples_ms": values,
        "median_ms": median(values),
        "min_ms": values.iter().min().unwrap(),
        "max_ms": values.iter().max().unwrap(),
    })
}

fn run_case(
    tier: usize,
    contract_calls: usize,
    samples: usize,
    permutations_per_call: usize,
) -> Value {
    let base_slots = (tier + 1) * TX_SPINE_SLOTS;
    let object_slots = contract_calls * permutations_per_call;
    let live_slots = base_slots + object_slots;
    let slot_envelope = live_slots.next_power_of_two();
    let num_vars = num_vars_for(live_slots);
    let field_cells = 1usize << num_vars;
    let inputs = slot_inputs(live_slots, 0x5A11_0C00 + tier as u128);

    let build_started = Instant::now();
    let mle = BlockSpineMle::build_from_slot_state_ins_padded(&inputs, live_slots);
    let build_ms = build_started.elapsed().as_millis();
    assert_eq!(mle.live_slots, live_slots);
    assert_eq!(mle.num_vars, num_vars);

    // One unmeasured warm-up runs the same complete main+shift transcript.
    let mut warm_channel = Poseidon2bChannel::new();
    let (warm_main, warm_r) = prove_block_spine_unified(&mle, &mut warm_channel);
    let warm_stub = BlockSpineUnifiedReduction {
        r_prime: warm_r.clone(),
        s_in_dec_at_r: warm_main.s_in_dec_at_r,
        s_out_dec_at_r: warm_main.s_out_dec_at_r,
        state_dec_at_r: warm_main.state_dec_at_r,
        state_at_r: warm_main.state_at_r,
        s_out_lane_dec_at_r: warm_main.s_out_lane_dec_at_r,
        state_lane_dec_at_r: warm_main.state_lane_dec_at_r,
        beta: Block128::ZERO,
        gamma: Block128::ZERO,
    };
    let _ = prove_block_spine_shift(&mle, &warm_r, &warm_stub, &mut warm_channel);

    let mut main_ms = Vec::with_capacity(samples);
    let mut shift_ms = Vec::with_capacity(samples);
    let mut total_ms = Vec::with_capacity(samples);
    let mut verify_ms = Vec::with_capacity(samples);
    let mut proof_bytes = Vec::with_capacity(samples);
    for _ in 0..samples {
        let mut prover_channel = Poseidon2bChannel::new();
        let total_started = Instant::now();
        let main_started = Instant::now();
        let (main, r_prime) = prove_block_spine_unified(&mle, &mut prover_channel);
        main_ms.push(main_started.elapsed().as_millis());
        let stub = BlockSpineUnifiedReduction {
            r_prime: r_prime.clone(),
            s_in_dec_at_r: main.s_in_dec_at_r,
            s_out_dec_at_r: main.s_out_dec_at_r,
            state_dec_at_r: main.state_dec_at_r,
            state_at_r: main.state_at_r,
            s_out_lane_dec_at_r: main.s_out_lane_dec_at_r,
            state_lane_dec_at_r: main.state_lane_dec_at_r,
            beta: Block128::ZERO,
            gamma: Block128::ZERO,
        };
        let shift_started = Instant::now();
        let (shift, _) = prove_block_spine_shift(&mle, &r_prime, &stub, &mut prover_channel);
        shift_ms.push(shift_started.elapsed().as_millis());
        total_ms.push(total_started.elapsed().as_millis());
        proof_bytes.push(
            bincode::serialize(&(main.clone(), shift.clone()))
                .unwrap()
                .len(),
        );

        let verify_started = Instant::now();
        let mut verifier_channel = Poseidon2bChannel::new();
        let main_reduction =
            verify_block_spine_unified(&main, num_vars, live_slots, &mut verifier_channel)
                .expect("honest augmented main proof");
        verify_block_spine_shift(&shift, &main_reduction, num_vars, &mut verifier_channel)
            .expect("honest augmented shift proof");
        verify_ms.push(verify_started.elapsed().as_millis());
    }
    assert!(proof_bytes.windows(2).all(|window| window[0] == window[1]));

    json!({
        "tier": tier,
        "contract_calls": contract_calls,
        "permutations_per_call": permutations_per_call,
        "tx_spine_slots": base_slots,
        "object_hash_slots": object_slots,
        "live_slots": live_slots,
        "slot_envelope": slot_envelope,
        "slot_slack_after": slot_envelope - live_slots,
        "num_vars": num_vars,
        "column_cells": field_cells,
        "mle_build_ms": build_ms,
        "main_prove": stats(&main_ms),
        "shift_prove": stats(&shift_ms),
        "combined_prove": stats(&total_ms),
        "native_verify": stats(&verify_ms),
        "serialized_main_plus_shift_bytes": proof_bytes[0],
        "same_dyadic_envelope_as_tier_baseline": slot_envelope == base_slots.next_power_of_two(),
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
    let args: Vec<_> = std::env::args().collect();
    let tier = args
        .get(1)
        .map(|value| value.parse::<usize>().expect("tier"))
        .unwrap_or(B25_TIER);
    assert!(tier == B25_TIER || tier == B255_TIER);
    let calls: Vec<usize> = if let Some(value) = args.get(2) {
        value
            .split(',')
            .map(|part| part.parse().expect("contract call count"))
            .collect()
    } else if tier == B25_TIER {
        vec![0, 1, 7, 25]
    } else {
        vec![0, 1, 64]
    };
    let samples = args
        .get(3)
        .map(|value| value.parse::<usize>().expect("sample count"))
        .unwrap_or(if tier == B25_TIER { 3 } else { 1 });
    assert!(samples > 0);
    let permutations_per_call = std::env::var("NOID_V2_PERMUTATIONS_PER_CALL")
        .ok()
        .map(|value| value.parse::<usize>().expect("permutations per call"))
        .unwrap_or(DEFAULT_OBJECT_PERMUTATIONS_PER_CALL);
    assert!(permutations_per_call > 0);

    let base_slots = (tier + 1) * TX_SPINE_SLOTS;
    let envelope = base_slots.next_power_of_two();
    let maximum_calls_without_envelope_growth = (envelope - base_slots) / permutations_per_call;
    for &count in &calls {
        assert!(count <= maximum_calls_without_envelope_growth);
    }
    let cases = calls
        .iter()
        .map(|&contract_calls| run_case(tier, contract_calls, samples, permutations_per_call))
        .collect::<Vec<_>>();

    let result = json!({
        "kind": "v2-augmented-block-spine-timing",
        "status": "native-main-and-shift-measured-integration-not-established",
        "source_revision": source_revision(),
        "generated_unix_seconds": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "rayon_threads": std::env::var("RAYON_NUM_THREADS").ok(),
        "tier": tier,
        "tier_body_instances": tier + 1,
        "baseline_tx_spine_slots": base_slots,
        "dyadic_slot_envelope": envelope,
        "unused_permutation_slots_before_contracts": envelope - base_slots,
        "object_permutations_per_contract_call": permutations_per_call,
        "maximum_calls_without_envelope_growth": maximum_calls_without_envelope_growth,
        "physical_user_page_limit": tier,
        "cases": cases,
        "established": [
            "The configured program/object hashing budget is represented by the exact measured number of appended Poseidon2b permutation slots per call.",
            "The tested augmented slot counts remain in the same production block-spine dyadic envelope.",
            "Production block-spine main and shift sumchecks accept the augmented arbitrary permutation slots.",
            "Main-plus-shift proof byte length is unchanged while num_vars is unchanged.",
        ],
        "not_established": [
            "A mixed Tx8x2/object descriptor that derives every object slot input from capsule statement wires.",
            "Four final old/new root pins per call against the selected transaction input and output owners.",
            "Recursive trace rows for the mixed descriptor and pins.",
            "Shared PCS opening, region-sidecar integration, or complete HistoryStep timing.",
            "Network serialization or propagation impact from public contract statement fields.",
        ],
    });
    let encoded = serde_json::to_vec_pretty(&result).unwrap();
    if let Some(path) = args.get(4) {
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
