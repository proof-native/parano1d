//! Stage-zero measurements, NOT a contract VM or a v2 consensus implementation.
//! Measures closed native C1 proofs for hash-heavy proxy relations and unchanged
//! wallet capsules. Neither path measures recursively admitting a contract.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use noid_core::Block128;
use noid_gkr::{
    prove_paged_spend_authorization, verify_paged_spend_authorization, OwnerAuthWitness,
    WalletAuthorizationBundle,
};
use noid_ivc_core::challenger::FsLaneChallenger;
use noid_ivc_core::field::F128;
use noid_ivc_core::field_circuit::{flat_const, poseidon2b_permute, FieldR1csBuilder, LinExpr};
use noid_ivc_core::pcs::{self, PcsParams};
use noid_ivc_core::verifier::verify_field_c1;
use noid_ivc_prover::field_prover::prove_field_c1;
use noid_poseidon2b::native::permutation::Poseidon2bPermutation;
use noid_tx::{PagedSpendIntent, TxPage, PAGED_SPEND_END_BIT, PAGED_SPEND_START_BIT};
use serde_json::{json, Value};

const DOMAIN: &[u8] = b"NOID/RESEARCH/V2-FEASIBILITY/STAGE0/C1";

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
    let values: Vec<_> = samples
        .iter()
        .map(|row| row[key].as_f64().unwrap())
        .collect();
    json!({"min": percentile(&values, 1), "median": percentile(&values, 50),
        "max": percentile(&values, 100)})
}

fn substrate(hashes: usize, copies: usize, samples: usize) -> Value {
    assert!((1..=2048).contains(&hashes) && (1..=16).contains(&copies));
    assert!(hashes * copies <= 2048, "keep this exploratory run bounded");
    let started = Instant::now();
    let mut builder = FieldR1csBuilder::new();
    for copy in 0..copies {
        let seed: [Block128; 4] =
            std::array::from_fn(|i| Block128(0x1000 + 100 * copy as u128 + i as u128));
        // Public seed and terminal are pinned into this reference matrix.
        // Thus it is NOT a reusable, program-authenticated universal verifier.
        let mut state: [LinExpr; 4] = std::array::from_fn(|i| {
            LinExpr::from_wire(builder.alloc_public_f128(flat_const(seed[i].0)))
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
    let useful_rows = builder.num_wires();
    let (matrix, witness) = builder.build();
    let build_ms = ms(started);
    assert!(matrix.satisfies(&witness));

    let params = PcsParams {
        m: matrix.m + pcs::LOG_PACKING,
        log_inv_rate: 2,
        log_batch_size: 5,
        profile: Default::default(),
    };
    let started = Instant::now();
    let _ = matrix.statement_digest();
    let _ = matrix.csc_lincheck_circuit();
    let shape_setup_ms = ms(started);
    let mut timings = Vec::new();
    let mut queries = 0;
    let mut proof_bytes = 0;
    let mut payload_bytes = 0;
    let mut negative_tests = 0;
    for sample in 0..=samples {
        let started = Instant::now();
        let (proof, commitment, claim) = prove_field_c1(
            &matrix,
            &witness,
            &params,
            &mut FsLaneChallenger::new_c1(DOMAIN),
        );
        let prove_ms = ms(started);
        let started = Instant::now();
        let accepted = verify_field_c1(
            &matrix,
            &commitment,
            &proof,
            &mut FsLaneChallenger::new_c1(DOMAIN),
        )
        .expect("closed native C1 verifier must accept the honest relation");
        let verify_ms = ms(started);
        assert_eq!(accepted, claim);
        queries = proof.pcs_open.queries.len();
        assert_eq!(queries, pcs::BASEFOLD_RATE_QUARTER_C1_QUERIES);
        proof_bytes = bincode::serialized_size(&proof).unwrap() as usize;
        payload_bytes = bincode::serialized_size(&(&commitment, &proof)).unwrap() as usize;
        if sample == 0 {
            let mut corrupted = proof.clone();
            corrupted.zerocheck.final_a_eval.hi.lo ^= 1;
            assert!(verify_field_c1(
                &matrix,
                &commitment,
                &corrupted,
                &mut FsLaneChallenger::new_c1(DOMAIN)
            )
            .is_err());
            assert!(verify_field_c1(
                &matrix,
                &commitment,
                &proof,
                &mut FsLaneChallenger::new_c1(b"WRONG-RESEARCH-DOMAIN")
            )
            .is_err());
            let mut corrupted_witness = witness.clone();
            corrupted_witness[5] += F128::ONE;
            assert!(!matrix.satisfies(&corrupted_witness));
            negative_tests = 3;
        } else {
            timings.push(json!({"prove_ms": prove_ms, "verify_ms": verify_ms}));
        }
    }
    json!({
        "kind": "hash_proxy_closed_native_c1_not_recursive_contract",
        "hashes_per_copy": hashes, "copies_in_one_proof": copies,
        "useful_rows": useful_rows, "field_rows_log2": matrix.m,
        "padded_rows": 1usize << matrix.m, "witness_bytes": witness.len() * 16,
        "codeword_bytes": params.codeword_len_f128() * 16,
        "matrix_nnz": matrix.a_0.nnz() + matrix.b_0.nnz(),
        "queries": queries, "log_inv_rate": 2, "log_batch_size": 5,
        "proof_bytes_bincode_expanded": proof_bytes,
        "commitment_and_proof_bytes_bincode_expanded": payload_bytes,
        "shape_build_ms": build_ms, "shape_preparation_ms": shape_setup_ms,
        "warmup_samples": 1, "measured_samples": samples,
        "prove_ms": summary(&timings, "prove_ms"),
        "verify_ms": summary(&timings, "verify_ms"),
        "negative_checks_passed": negative_tests, "raw_samples": timings,
        "exclusions": ["FROST-GKR batching", "program authentication/interpreter",
            "ZK for arbitrary witnesses", "HistoryStep recursive integration",
            "State reads and writes", "network and storage", "consensus admission"]
    })
}

fn pages_for(index: usize) -> Vec<TxPage> {
    let mut scenario = bench_prover::tx8x2_scenario(
        "v2-wallet-proxy",
        1,
        1,
        10_000 * index as u32 + 1,
        0x2000 + index as u128,
    );
    scenario.body.validity_bitmap |= PAGED_SPEND_START_BIT | PAGED_SPEND_END_BIT;
    vec![TxPage::new(scenario.body).unwrap()]
}

fn wallet(copies: usize, samples: usize) -> Value {
    assert!((1..=25).contains(&copies));
    let pages: Vec<_> = (0..copies).map(pages_for).collect();
    let mut timings = Vec::new();
    let mut proof_bytes = 0;
    let mut intent_bytes = 0;
    for sample in 0..=samples {
        let started = Instant::now();
        let bundles: Vec<_> = pages
            .iter()
            .enumerate()
            .map(|(i, pages)| {
                prove_paged_spend_authorization(
                    pages,
                    OwnerAuthWitness::new(bench_prover::mk_secret(0x2000 + i as u128)),
                )
                .expect("valid wallet authorization")
            })
            .collect();
        let sequential_prove_ms = ms(started);
        let started = Instant::now();
        let wire: Vec<_> = pages
            .iter()
            .zip(&bundles)
            .map(|(pages, bundle)| {
                PagedSpendIntent::new(pages.clone(), bundle.to_bytes().unwrap())
                    .unwrap()
                    .to_bytes()
                    .unwrap()
            })
            .collect();
        let encode_ms = ms(started);
        let started = Instant::now();
        for bytes in &wire {
            let intent = PagedSpendIntent::from_bytes(bytes).unwrap();
            let bundle =
                WalletAuthorizationBundle::from_bytes(&intent.authorization_bytes).unwrap();
            verify_paged_spend_authorization(&intent.pages, &bundle).unwrap();
        }
        let sequential_decode_verify_ms = ms(started);
        proof_bytes = bundles
            .iter()
            .map(|b| b.to_bytes().unwrap().len())
            .sum::<usize>();
        intent_bytes = wire.iter().map(Vec::len).sum::<usize>();
        if sample == 0 {
            let mut changed = pages[0].clone();
            changed[0].body.outputs[0].owner.0[0] ^= 1;
            assert!(verify_paged_spend_authorization(&changed, &bundles[0]).is_err());
        } else {
            timings.push(json!({"sequential_prove_ms": sequential_prove_ms,
                "encode_ms": encode_ms, "sequential_decode_verify_ms": sequential_decode_verify_ms,
                "authorization_bytes": proof_bytes, "intent_bytes": intent_bytes}));
        }
    }
    json!({"kind": "independent_existing_wallet_capsules_not_multi_owner_transaction",
        "independent_capsules": copies, "measured_samples": samples, "warmup_samples": 1,
        "total_authorization_bytes_last_sample": proof_bytes, "total_intent_bytes_last_sample": intent_bytes,
        "authorization_bytes": summary(&timings, "authorization_bytes"),
        "intent_bytes": summary(&timings, "intent_bytes"),
        "sequential_prove_ms": summary(&timings, "sequential_prove_ms"),
        "encode_ms": summary(&timings, "encode_ms"),
        "sequential_decode_verify_ms": summary(&timings, "sequential_decode_verify_ms"),
        "raw_samples": timings, "recipient_mutation_rejected": true,
        "exclusions": ["parallel proving across owners", "shared authorization proof",
            "atomic multi-owner transaction", "recursive verification in HistoryStep",
            "network and storage"]})
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(args.len() == 5 || args.len() == 6,
        "usage: paranoid-v2-feasibility substrate HASHES COPIES SAMPLES [NEW_JSON_FILE]\n       paranoid-v2-feasibility wallet 0 COPIES SAMPLES [NEW_JSON_FILE]");
    let hashes: usize = args[2].parse().unwrap();
    let copies: usize = args[3].parse().unwrap();
    let samples: usize = args[4].parse().unwrap();
    assert!((1..=20).contains(&samples));
    noid_ivc_core::init_perf_thread_pool();
    let result = match args[1].as_str() {
        "substrate" => substrate(hashes, copies, samples),
        "wallet" => wallet(copies, samples),
        _ => panic!("unknown measurement kind"),
    };
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let cpu = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let cpu = cpu
        .lines()
        .find(|line| line.starts_with("model name"))
        .unwrap_or("unknown");
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let hwm = status
        .lines()
        .find(|line| line.starts_with("VmHWM:"))
        .unwrap_or("unknown");
    let record = json!({"schema": 1, "source_revision": String::from_utf8(revision.stdout).unwrap().trim(),
        "unix_time": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "cpu": cpu, "backend": noid_core::cpu::ProductionHardwareReport::detect().backend.to_string(),
        "rayon_threads": rayon::current_num_threads(), "whole_process_peak_rss": hwm,
        "result": result});
    let encoded = serde_json::to_string_pretty(&record).unwrap();
    if let Some(path) = args.get(5) {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("result path must be new, existing measurements are never overwritten");
        writeln!(file, "{encoded}").unwrap();
    }
    println!("{encoded}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_summary() {
        assert_eq!(percentile(&[9.0, 1.0, 5.0], 50), 5.0);
        assert_eq!(percentile(&[9.0, 1.0, 5.0], 100), 9.0);
    }

    #[test]
    fn fixture_has_one_canonical_logical_spend() {
        for i in 0..25 {
            let facts = noid_tx::validate_paged_spend(&pages_for(i)).unwrap();
            assert_eq!(facts.live_inputs, 1);
            assert_eq!(facts.live_outputs, 1);
        }
    }
}
