//! Research-only recursive replay measurement.
//!
//! This is a real C1 proof whose relation replays one complete inner C1
//! verifier. It is NOT a contract implementation or a HistoryStep change.
//! The compiled inner relation is fixed in the outer matrix and has no dynamic
//! public State-effect envelope. A second, path-free trace is measured only to
//! decompose cost; its obligations are deliberately not treated as a proof.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use noid_core::Block128;
use noid_ivc_core::challenger::FsLaneChallenger;
use noid_ivc_core::field::F128;
use noid_ivc_core::field_circuit::{
    flat_const, poseidon2b_permute, FieldR1csBuilder, FsChannelTrace, LinExpr,
};
use noid_ivc_core::field_r1cs::FieldR1cs;
use noid_ivc_core::pcs::{self, PcsParams};
use noid_ivc_core::verifier::verify_field_c1;
use noid_ivc_prover::field_prover::prove_field_c1;
use noid_poseidon2b::native::permutation::Poseidon2bPermutation;
use noid_recursive::acceptance::trace::self_verify::{
    alloc_flat_digest, verify_field_c1_trace, verify_field_c1_trace_region, C1FieldR1csProofTrace,
    PcsWalkObligations,
};
use serde_json::{json, Value};

const INNER_DOMAIN: &[u8] = b"NOID/RESEARCH/V2-FEASIBILITY/COMPILED-INNER-C1";
const OUTER_DOMAIN: &[u8] = b"NOID/RESEARCH/V2-FEASIBILITY/RECURSIVE-OUTER-C1";

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn percentile(values: &[f64], p: usize) -> f64 {
    assert!(!values.is_empty() && (1..=100).contains(&p));
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[(p * sorted.len()).div_ceil(100) - 1]
}

fn summary(samples: &[Value], key: &str) -> Value {
    let values = samples
        .iter()
        .map(|sample| sample[key].as_f64().unwrap())
        .collect::<Vec<_>>();
    json!({
        "min": percentile(&values, 1),
        "median": percentile(&values, 50),
        "max": percentile(&values, 100),
    })
}

fn compiled_relation(hashes: usize, copies: usize) -> (FieldR1cs, Vec<F128>) {
    assert!((1..=2048).contains(&hashes));
    assert!((1..=16).contains(&copies));
    assert!(hashes * copies <= 2048, "keep the research run bounded");
    let mut builder = FieldR1csBuilder::new();
    for copy in 0..copies {
        let seed: [Block128; 4] =
            std::array::from_fn(|lane| Block128(0x7000 + 100 * copy as u128 + lane as u128));
        let mut state: [LinExpr; 4] = std::array::from_fn(|lane| {
            LinExpr::from_wire(builder.alloc_public_f128(flat_const(seed[lane].0)))
        });
        let mut expected = seed;
        for _ in 0..hashes {
            state = poseidon2b_permute(&mut builder, state);
            Poseidon2bPermutation.permute_mut(&mut expected);
        }
        for (lane, native) in state.iter().zip(expected) {
            builder.pin_f128(lane, flat_const(native.0));
        }
    }
    let (relation, witness) = builder.build();
    assert!(relation.satisfies(&witness));
    (relation, witness)
}

fn params(relation: &FieldR1cs) -> PcsParams {
    PcsParams {
        m: relation.m + pcs::LOG_PACKING,
        log_inv_rate: 2,
        log_batch_size: 5,
        profile: Default::default(),
    }
}

fn full_replay_relation(
    inner: &FieldR1cs,
    inner_params: &PcsParams,
    commitment: &pcs::Commitment,
    proof: &noid_ivc_core::proof::C1FieldR1csProof,
) -> (FieldR1cs, Vec<F128>, usize, usize) {
    let mut builder = FieldR1csBuilder::new();
    let mut channel = FsChannelTrace::new_c1(&mut builder, INNER_DOMAIN);
    let root = alloc_flat_digest(&mut builder, &commitment.root);
    let proof_start = builder.num_wires();
    let proof_trace = C1FieldR1csProofTrace::alloc(&mut builder, proof, inner, inner_params);
    let proof_wires = builder.num_wires() - proof_start;
    let verifier_start = builder.num_wires();
    let _ = verify_field_c1_trace(
        &mut builder,
        &mut channel,
        inner,
        inner_params,
        &root,
        &proof_trace,
    );
    let verifier_rows = builder.num_wires() - verifier_start;
    let (relation, witness) = builder.build();
    assert!(relation.satisfies(&witness));
    (relation, witness, proof_wires, verifier_rows)
}

fn path_free_decomposition(
    inner: &FieldR1cs,
    inner_params: &PcsParams,
    commitment: &pcs::Commitment,
    proof: &noid_ivc_core::proof::C1FieldR1csProof,
) -> Value {
    let mut builder = FieldR1csBuilder::new();
    let mut channel = FsChannelTrace::new_c1(&mut builder, INNER_DOMAIN);
    let root = alloc_flat_digest(&mut builder, &commitment.root);
    let proof_start = builder.num_wires();
    let shape = noid_ivc_core::proof::FieldShape::of(inner);
    let proof_trace =
        C1FieldR1csProofTrace::alloc_shape_mode(&mut builder, proof, &shape, inner_params, false);
    let proof_wires = builder.num_wires() - proof_start;
    let verifier_start = builder.num_wires();
    let mut obligations = PcsWalkObligations::default();
    let _ = verify_field_c1_trace_region(
        &mut builder,
        &mut channel,
        inner,
        inner_params,
        &root,
        &proof_trace,
        Some(&mut obligations),
    );
    let verifier_rows = builder.num_wires() - verifier_start;
    let useful_rows = builder.num_wires();
    let leaf_lanes = obligations
        .leaves
        .iter()
        .map(|obligation| obligation.lanes.len())
        .sum::<usize>();
    let path_levels = obligations
        .paths
        .iter()
        .map(|obligation| obligation.dir_bits.len())
        .sum::<usize>();
    let (relation, witness) = builder.build();
    assert!(relation.satisfies(&witness));
    json!({
        "status": "incomplete_until_every_recorded_obligation_is_discharged",
        "useful_rows": useful_rows,
        "padded_rows": 1usize << relation.m,
        "proof_wires_without_paths": proof_wires,
        "verifier_rows_without_leaf_and_path_hashing": verifier_rows,
        "leaf_obligations": obligations.leaves.len(),
        "path_obligations": obligations.paths.len(),
        "total_leaf_lanes": leaf_lanes,
        "total_path_levels": path_levels,
    })
}

fn run(hashes: usize, copies: usize, samples: usize) -> Value {
    let started = Instant::now();
    let (inner, inner_witness) = compiled_relation(hashes, copies);
    let inner_build_ms = ms(started);
    let inner_params = params(&inner);

    let started = Instant::now();
    let (inner_proof, inner_commitment, _) = prove_field_c1(
        &inner,
        &inner_witness,
        &inner_params,
        &mut FsLaneChallenger::new_c1(INNER_DOMAIN),
    );
    let inner_prove_ms = ms(started);
    verify_field_c1(
        &inner,
        &inner_commitment,
        &inner_proof,
        &mut FsLaneChallenger::new_c1(INNER_DOMAIN),
    )
    .expect("native verifier accepts the compiled inner proof");

    let path_free = path_free_decomposition(&inner, &inner_params, &inner_commitment, &inner_proof);
    let started = Instant::now();
    let (outer, outer_witness, proof_wires, verifier_rows) =
        full_replay_relation(&inner, &inner_params, &inner_commitment, &inner_proof);
    let full_trace_build_ms = ms(started);
    let outer_params = params(&outer);
    let mut timings = Vec::new();
    let mut outer_proof_bytes = 0usize;
    let mut negative_checks = 0usize;
    for sample in 0..=samples {
        let started = Instant::now();
        let (proof, commitment, _) = prove_field_c1(
            &outer,
            &outer_witness,
            &outer_params,
            &mut FsLaneChallenger::new_c1(OUTER_DOMAIN),
        );
        let prove_ms = ms(started);
        let started = Instant::now();
        verify_field_c1(
            &outer,
            &commitment,
            &proof,
            &mut FsLaneChallenger::new_c1(OUTER_DOMAIN),
        )
        .expect("native verifier accepts the outer recursive replay proof");
        let verify_ms = ms(started);
        outer_proof_bytes = bincode::serialized_size(&proof).unwrap() as usize;
        if sample == 0 {
            let mut corrupted = proof.clone();
            corrupted.zerocheck.final_c_eval.hi.hi ^= 1;
            assert!(verify_field_c1(
                &outer,
                &commitment,
                &corrupted,
                &mut FsLaneChallenger::new_c1(OUTER_DOMAIN),
            )
            .is_err());
            negative_checks = 1;
        } else {
            timings.push(json!({"outer_prove_ms": prove_ms, "outer_verify_ms": verify_ms}));
        }
    }
    json!({
        "kind": "complete_inline_recursive_replay_of_fixed_compiled_c1_relation",
        "hashes_per_copy": hashes,
        "copies_batched_in_inner_proof": copies,
        "inner_useful_rows": inner.useful_rows,
        "inner_padded_rows": 1usize << inner.m,
        "inner_build_ms": inner_build_ms,
        "inner_prove_ms_one_setup_sample": inner_prove_ms,
        "inner_proof_bytes_bincode_expanded": bincode::serialized_size(&inner_proof).unwrap(),
        "full_replay": {
            "useful_rows": outer.useful_rows,
            "padded_rows": 1usize << outer.m,
            "allocated_proof_wires": proof_wires,
            "verifier_rows": verifier_rows,
            "trace_build_and_satisfiability_ms": full_trace_build_ms,
            "outer_proof_bytes_bincode_expanded": outer_proof_bytes,
            "outer_prove_ms": summary(&timings, "outer_prove_ms"),
            "outer_verify_ms": summary(&timings, "outer_verify_ms"),
        },
        "path_free_cost_decomposition_not_a_proof": path_free,
        "warmup_samples": 1,
        "measured_samples": samples,
        "negative_checks_passed": negative_checks,
        "raw_samples": timings,
        "security_boundary": "full_replay includes inline PCS leaf and Merkle path checks",
        "exclusions": [
            "dynamic public State-effect envelope",
            "permissionless program or matrix authentication",
            "contract interpreter and integer semantics",
            "HistoryStep integration and its optimized region sidecar",
            "network, storage, contention and admission",
        ],
    })
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert!(
        args.len() == 4 || args.len() == 5,
        "usage: recursive_replay HASHES COPIES SAMPLES [NEW_JSON_FILE]"
    );
    let hashes = args[1].parse().unwrap();
    let copies = args[2].parse().unwrap();
    let samples = args[3].parse().unwrap();
    assert!((1..=5).contains(&samples));
    noid_ivc_core::init_perf_thread_pool();
    let result = run(hashes, copies, samples);
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let record = json!({
        "schema": 1,
        "source_revision": String::from_utf8(revision.stdout).unwrap().trim(),
        "unix_time": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "backend": noid_core::cpu::ProductionHardwareReport::detect().backend.to_string(),
        "rayon_threads": rayon::current_num_threads(),
        "whole_process_peak_rss": status.lines().find(|line| line.starts_with("VmHWM:")).unwrap_or("unknown"),
        "result": result,
    });
    let encoded = serde_json::to_string_pretty(&record).unwrap();
    if let Some(path) = args.get(4) {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("result path must be new");
        writeln!(file, "{encoded}").unwrap();
    }
    println!("{encoded}");
}
