//! Research-only fixed-shape interpreter floor.
//!
//! The relation accepts dynamic opcode/immediate witnesses under one unchanged
//! matrix, commits to the full fixed-size program and binds its final value.
//! It deliberately omits memory, branches, integers, object access and the
//! surrounding HistoryStep links. Its row count is therefore optimistic.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use noid_ivc_core::challenger::FsLaneChallenger;
use noid_ivc_core::field::F128;
use noid_ivc_core::field_circuit::{flat_const, poseidon2b_permute, FieldR1csBuilder, LinExpr};
use noid_ivc_core::field_r1cs::FieldR1cs;
use noid_ivc_core::pcs::{self, PcsParams};
use noid_ivc_core::verifier::verify_field_c1;
use noid_ivc_prover::field_prover::prove_field_c1;
use noid_recursive::acceptance::trace::{mul, pin_eq};
use serde_json::{json, Value};

const DOMAIN: &[u8] = b"NOID/RESEARCH/V2-FEASIBILITY/BOUNDED-INTERPRETER-C1";
const B25_USEFUL_ROWS: usize = 4_185_273;
const B25_PADDED_ROWS: usize = 1 << 22;

struct InterpreterFixture {
    relation: FieldR1cs,
    witness: Vec<F128>,
    first_opcode_wire: usize,
    declared_final_wire: usize,
}

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

fn select(builder: &mut FieldR1csBuilder, bit: &LinExpr, zero: &LinExpr, one: &LinExpr) -> LinExpr {
    zero.add(&mul(builder, bit, &zero.add(one)))
}

fn fixture(slots: usize, active_steps: usize, variant: usize) -> InterpreterFixture {
    assert!((1..=256).contains(&slots));
    assert!(active_steps <= slots);
    let mut builder = FieldR1csBuilder::new();

    // These are witness values. In a real HistoryStep they must be linked to
    // the authenticated input object and output effect.
    let old_value = flat_const(0xCAFEu128 + variant as u128);
    let mut accumulator = LinExpr::from_wire(builder.alloc_f128(old_value));
    let mut program_hash = [
        LinExpr::constant(flat_const(0x5052_4f47_5241_4du128)),
        LinExpr::zero(),
        LinExpr::zero(),
        LinExpr::zero(),
    ];
    let mut first_opcode_wire = None;

    for step in 0..slots {
        let opcode = if step < active_steps {
            (step + variant) & 3
        } else {
            0
        };
        let bit0 = builder.alloc_bool(opcode & 1 != 0);
        let bit1 = builder.alloc_bool(opcode & 2 != 0);
        first_opcode_wire.get_or_insert(bit0.0 as usize);
        let bit0 = LinExpr::from_wire(bit0);
        let bit1 = LinExpr::from_wire(bit1);
        let immediate_value = if step < active_steps {
            flat_const(0x100u128 + (variant * slots + step + 1) as u128)
        } else {
            F128::ZERO
        };
        let immediate = LinExpr::from_wire(builder.alloc_f128(immediate_value));

        // 00 NOP, 01 ADD, 10 MUL, 11 SQUARE+IMMEDIATE. Selection and opcode
        // checks have fixed shape regardless of the program witness.
        let add = accumulator.add(&immediate);
        let product = mul(&mut builder, &accumulator, &immediate);
        let square_add = mul(&mut builder, &accumulator, &accumulator).add(&immediate);
        let low = select(&mut builder, &bit0, &accumulator, &add);
        let high = select(&mut builder, &bit0, &product, &square_add);
        accumulator = select(&mut builder, &bit1, &low, &high);

        // Commit to every code slot. This makes unused capacity non-free and
        // keeps the matrix universal across program values.
        program_hash[0] = program_hash[0].add(&bit0);
        program_hash[1] = program_hash[1].add(&bit1);
        program_hash[2] = program_hash[2].add(&immediate);
        program_hash[3] = program_hash[3].add_const(flat_const((step + 1) as u128));
        program_hash = poseidon2b_permute(&mut builder, program_hash);
    }

    // Declared lanes stay witness variables, so their values do not alter the
    // matrix. The equalities model links to an enclosing block/object trace.
    for digest in &program_hash {
        let declared = LinExpr::from_wire(builder.alloc_f128(digest.eval(builder.values())));
        pin_eq(&mut builder, digest, &declared);
    }
    let declared_final = builder.alloc_f128(accumulator.eval(builder.values()));
    pin_eq(
        &mut builder,
        &accumulator,
        &LinExpr::from_wire(declared_final),
    );

    let (relation, witness) = builder.build();
    assert!(relation.satisfies(&witness));
    InterpreterFixture {
        relation,
        witness,
        first_opcode_wire: first_opcode_wire.unwrap(),
        declared_final_wire: declared_final.0 as usize,
    }
}

fn run(slots: usize, active_steps: usize, samples: usize) -> Value {
    let started = Instant::now();
    let candidate = fixture(slots, active_steps, 0);
    let build_ms = ms(started);
    let alternate = fixture(slots, active_steps.saturating_sub(1), 1);
    assert_eq!(candidate.relation.m, alternate.relation.m);
    assert_eq!(
        candidate.relation.useful_rows,
        alternate.relation.useful_rows
    );
    assert_eq!(
        candidate.relation.statement_digest(),
        alternate.relation.statement_digest(),
        "different program witnesses must use exactly one matrix"
    );

    let mut bad_opcode = candidate.witness.clone();
    bad_opcode[candidate.first_opcode_wire] = F128 { lo: 2, hi: 0 };
    assert!(!candidate.relation.satisfies(&bad_opcode));
    let mut bad_effect = candidate.witness.clone();
    bad_effect[candidate.declared_final_wire] += F128::ONE;
    assert!(!candidate.relation.satisfies(&bad_effect));

    let params = PcsParams {
        m: candidate.relation.m + pcs::LOG_PACKING,
        log_inv_rate: 2,
        log_batch_size: 5,
        profile: Default::default(),
    };
    let mut timings = Vec::new();
    let mut proof_bytes = 0usize;
    let mut negative_checks = 2usize;
    for sample in 0..=samples {
        let started = Instant::now();
        let (proof, commitment, _) = prove_field_c1(
            &candidate.relation,
            &candidate.witness,
            &params,
            &mut FsLaneChallenger::new_c1(DOMAIN),
        );
        let prove_ms = ms(started);
        let started = Instant::now();
        verify_field_c1(
            &candidate.relation,
            &commitment,
            &proof,
            &mut FsLaneChallenger::new_c1(DOMAIN),
        )
        .expect("honest bounded interpreter proof verifies");
        let verify_ms = ms(started);
        proof_bytes = bincode::serialized_size(&proof).unwrap() as usize;
        if sample == 0 {
            let mut corrupted = proof.clone();
            corrupted.zerocheck.final_a_eval.lo.lo ^= 1;
            assert!(verify_field_c1(
                &candidate.relation,
                &commitment,
                &corrupted,
                &mut FsLaneChallenger::new_c1(DOMAIN),
            )
            .is_err());
            negative_checks += 1;
        } else {
            timings.push(json!({"prove_ms": prove_ms, "verify_ms": verify_ms}));
        }
    }

    let incremental_rows = candidate.relation.useful_rows - 1;
    let combined_useful_rows = B25_USEFUL_ROWS + incremental_rows;
    let combined_padded_rows = combined_useful_rows.next_power_of_two();
    json!({
        "kind": "optimistic_fixed_shape_dynamic_opcode_interpreter_floor",
        "slots": slots,
        "active_steps": active_steps,
        "useful_rows": candidate.relation.useful_rows,
        "padded_rows": 1usize << candidate.relation.m,
        "incremental_rows_if_appended_without_reuse": incremental_rows,
        "row_sum_projection_with_existing_b25": {
            "existing_useful_rows": B25_USEFUL_ROWS,
            "existing_padded_rows": B25_PADDED_ROWS,
            "combined_useful_rows": combined_useful_rows,
            "combined_padded_rows": combined_padded_rows,
            "geometry_multiplier": combined_padded_rows as f64 / B25_PADDED_ROWS as f64,
            "status": "row_arithmetic_only_not_an_integrated_history_step_benchmark",
        },
        "same_matrix_digest_for_distinct_program_and_state_witnesses": true,
        "shape_and_witness_build_ms": build_ms,
        "proof_bytes_bincode_expanded": proof_bytes,
        "prove_ms": summary(&timings, "prove_ms"),
        "verify_ms": summary(&timings, "verify_ms"),
        "warmup_samples": 1,
        "measured_samples": samples,
        "negative_checks_passed": negative_checks,
        "raw_samples": timings,
        "included": [
            "two boolean opcode bits per slot",
            "dynamic NOP ADD MUL and SQUARE+IMMEDIATE selection",
            "full-program Poseidon2b commitment",
            "old-to-new value execution and declared-effect equality",
        ],
        "exclusions": [
            "branches and control-flow soundness",
            "memory and dynamic addressing",
            "bounded integer semantics and overflow",
            "multiple objects, authorization and origin rules",
            "linking declared program and effects to an authenticated block",
            "HistoryStep integration, network, storage and contention",
        ],
    })
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert!(
        args.len() == 4 || args.len() == 5,
        "usage: bounded_interpreter SLOTS ACTIVE_STEPS SAMPLES [NEW_JSON_FILE]"
    );
    let slots = args[1].parse().unwrap();
    let active_steps = args[2].parse().unwrap();
    let samples = args[3].parse().unwrap();
    assert!((1..=10).contains(&samples));
    noid_ivc_core::init_perf_thread_pool();
    let result = run(slots, active_steps, samples);
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
