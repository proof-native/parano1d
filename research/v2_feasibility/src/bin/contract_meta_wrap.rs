//! Research-only proof that a bounded contract commitment schedule fits in
//! the unused tail of the existing B25 Meta-A Tx-body block without changing
//! the walk domain or proof shape.
//!
//! The current per-transaction Meta-A block has 64 slots: body tree 0..32,
//! Tx8x2 wrap at 32, and padding 33..64.  We extend the *existing* wrap
//! sponge family over slots 33..47.  Its four fixed patterns, eight relation
//! terms, committed-column refs and shift claims stay exactly unchanged.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use noid_ivc_core::challenger::{Challenger, FsLaneChallenger};
use noid_ivc_core::deep_chain::leaf_hash::{sponge_leaf_substitution_terms, SpongeLeafRefs};
use noid_ivc_core::deep_chain::relations::{
    claimed_refs, prove_column_relation, prove_shift_discharge_pow2, verify_column_relation,
    verify_shift_discharge_pow2, ColRef, ColumnRelationProof, FixedPattern, RelationColumns,
    RelationTerm, ShiftDischargeProof,
};
use noid_ivc_core::deep_chain::schedule::carry_selection_terms;
use noid_ivc_core::deep_chain::source_tree::run_perm;
use noid_ivc_core::deep_chain::{
    prove_deep_chain_walk, verify_deep_chain_walk, DeepChainWalkProof, LaneClaimGroup,
};
use noid_ivc_core::field::F128;
use noid_ivc_core::lincheck::build_eq_table;
use noid_poseidon2b::native::domain::{capacity_iv_flat, DomainTag, TAG_TX8X2};
use serde_json::{json, Value};

const W_LOG: usize = 12;
const W: usize = 1 << W_LOG;
const META_HALF: usize = W / 2;
const TX_BLOCK_LOG: usize = 6;
const TX_BLOCK_SLOTS: usize = 1 << TX_BLOCK_LOG;
const TX_TILES: usize = 32;
const WRAP_SLOT: usize = 32;
const CONTRACT_BASE: usize = 33;
const CODE_SLOTS: usize = 8;
const OLD_OBJECT_SLOTS: usize = 3;
const NEW_OBJECT_SLOTS: usize = 3;
const CONTRACT_SLOTS: usize = CODE_SLOTS + OLD_OBJECT_SLOTS + NEW_OBJECT_SLOTS;
const CONTRACT_END: usize = CONTRACT_BASE + CONTRACT_SLOTS;
const REAL_CALLS: usize = 4;

const CODE_DOMAIN: DomainTag = DomainTag::new(b"CNTCODE_");
const OBJECT_DOMAIN: DomainTag = DomainTag::new(b"CNTOBJ__");
const OBJECT_VERSION: F128 = F128 { lo: 2, hi: 0 };

const IN0: usize = 0;
const C0: usize = 2;

#[derive(Clone)]
struct MetaColumns {
    committed: [Vec<F128>; 6],
    s0: [Vec<F128>; 4],
    s_out: [Vec<F128>; 4],
    input_pins: Vec<(usize, usize, F128)>,
    root_pins: Vec<(usize, [F128; 2])>,
    permutations_built_for_real_contracts: usize,
}

struct DagProof {
    selection: ColumnRelationProof,
    walk: DeepChainWalkProof,
    substitution: ColumnRelationProof,
    shifts: Vec<(usize, usize, ShiftDischargeProof)>,
}

fn f(index: usize, domain: u64) -> F128 {
    F128 {
        lo: domain
            .wrapping_add((index as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .rotate_left((index % 63) as u32),
        hi: domain.rotate_left(17) ^ (index as u64 + 11).wrapping_mul(0xBF58_476D_1CE4_E5B9),
    }
}

fn iv(tag: DomainTag) -> [F128; 2] {
    capacity_iv_flat(tag).map(|value| F128 {
        lo: value as u64,
        hi: (value >> 64) as u64,
    })
}

fn gated(mut pattern: FixedPattern) -> FixedPattern {
    // The wrap family occupies the second 2^11-slot half of the 2^12 Meta-A
    // walk.  Its 64-slot table repeats once per authorization tile.
    pattern = pattern.gated(11, vec![true]);
    pattern
}

fn fixed_patterns(contract: bool) -> Vec<FixedPattern> {
    let mut region = vec![F128::ZERO; TX_BLOCK_SLOTS];
    let mut carry = vec![F128::ZERO; TX_BLOCK_SLOTS];
    let mut iv0 = vec![F128::ZERO; TX_BLOCK_SLOTS];
    let mut iv1 = vec![F128::ZERO; TX_BLOCK_SLOTS];

    let wrap_iv = iv(TAG_TX8X2);
    region[WRAP_SLOT] = F128::ONE;
    iv0[WRAP_SLOT] = wrap_iv[0];
    iv1[WRAP_SLOT] = wrap_iv[1];

    if contract {
        region[CONTRACT_BASE..CONTRACT_END].fill(F128::ONE);
        let code_start = CONTRACT_BASE;
        let old_start = code_start + CODE_SLOTS;
        let new_start = old_start + OLD_OBJECT_SLOTS;
        for range in [
            code_start..code_start + CODE_SLOTS,
            old_start..old_start + OLD_OBJECT_SLOTS,
            new_start..new_start + NEW_OBJECT_SLOTS,
        ] {
            carry[range.start + 1..range.end].fill(F128::ONE);
        }
        let code_iv = iv(CODE_DOMAIN);
        let object_iv = iv(OBJECT_DOMAIN);
        for (slot, domain_iv) in [
            (code_start, code_iv),
            (old_start, object_iv),
            (new_start, object_iv),
        ] {
            iv0[slot] = domain_iv[0];
            iv1[slot] = domain_iv[1];
        }
    }

    vec![region, carry, iv0, iv1]
        .into_iter()
        .map(|table| gated(FixedPattern::new(TX_BLOCK_LOG, table)))
        .collect()
}

fn store(
    columns: &mut MetaColumns,
    slot: usize,
    absorbed: [F128; 2],
    prior: Option<[F128; 4]>,
    capacity: [F128; 2],
) -> [F128; 4] {
    let raw = match prior {
        None => [absorbed[0], absorbed[1], capacity[0], capacity[1]],
        Some(previous) => [
            previous[0] + absorbed[0],
            previous[1] + absorbed[1],
            previous[2],
            previous[3],
        ],
    };
    let (state_in, state_out) = run_perm(raw);
    columns.committed[IN0][slot] = absorbed[0];
    columns.committed[IN0 + 1][slot] = absorbed[1];
    for lane in 0..4 {
        columns.committed[C0 + lane][slot] = state_out[lane];
        columns.s0[lane][slot] = state_in[lane];
        columns.s_out[lane][slot] = state_out[lane];
    }
    state_out
}

fn build_columns(contract: bool) -> MetaColumns {
    let (ghost_s0, ghost_out) = run_perm([F128::ZERO; 4]);
    let mut columns = MetaColumns {
        committed: std::array::from_fn(|column| {
            if column >= C0 {
                vec![ghost_out[column - C0]; W]
            } else {
                vec![F128::ZERO; W]
            }
        }),
        s0: std::array::from_fn(|lane| vec![ghost_s0[lane]; W]),
        s_out: std::array::from_fn(|lane| vec![ghost_out[lane]; W]),
        input_pins: Vec::new(),
        root_pins: Vec::new(),
        permutations_built_for_real_contracts: 0,
    };
    let wrap_iv = iv(TAG_TX8X2);
    let code_iv = iv(CODE_DOMAIN);
    let object_iv = iv(OBJECT_DOMAIN);

    // Every tile already owns one Tx8x2 wrap.  The surrounding body-tree
    // root is represented by a deterministic pair in this isolated DAG.
    for tile in 0..TX_TILES {
        let block = META_HALF + tile * TX_BLOCK_SLOTS;
        let wrap_input = [f(tile * 2, 0x5752_4150), f(tile * 2 + 1, 0x5752_4150)];
        store(&mut columns, block + WRAP_SLOT, wrap_input, None, wrap_iv);

        if !contract {
            continue;
        }

        let real = tile < REAL_CALLS;
        let program: [[F128; 2]; CODE_SLOTS] = std::array::from_fn(|step| {
            if real {
                [
                    F128 {
                        lo: (step % 4) as u64,
                        hi: 0,
                    },
                    f(tile * CODE_SLOTS + step, 0xC0DE_0001),
                ]
            } else {
                [F128::ZERO; 2]
            }
        });
        let current = if real {
            f(tile, 0xC011_0BAD)
        } else {
            F128::ZERO
        };
        let next = if real {
            f(tile, 0xC011_600D)
        } else {
            F128::ZERO
        };
        let controller = if real {
            [f(tile * 2, 0xC017_2011), f(tile * 2 + 1, 0xC017_2011)]
        } else {
            [F128::ZERO; 2]
        };

        let code_start = block + CONTRACT_BASE;
        let mut previous = None;
        for (step, pair) in program.into_iter().enumerate() {
            let out = store(&mut columns, code_start + step, pair, previous, code_iv);
            previous = Some(out);
            if real {
                columns.input_pins.push((IN0, code_start + step, pair[0]));
                columns
                    .input_pins
                    .push((IN0 + 1, code_start + step, pair[1]));
            }
        }
        let code_digest = [previous.unwrap()[0], previous.unwrap()[1]];

        let old_start = code_start + CODE_SLOTS;
        let old0 = store(&mut columns, old_start, code_digest, None, object_iv);
        let old1 = store(
            &mut columns,
            old_start + 1,
            [current, controller[0]],
            Some(old0),
            object_iv,
        );
        let old2 = store(
            &mut columns,
            old_start + 2,
            [controller[1], OBJECT_VERSION],
            Some(old1),
            object_iv,
        );

        let new_start = old_start + OLD_OBJECT_SLOTS;
        let new0 = store(&mut columns, new_start, code_digest, None, object_iv);
        let new1 = store(
            &mut columns,
            new_start + 1,
            [next, controller[0]],
            Some(new0),
            object_iv,
        );
        let new2 = store(
            &mut columns,
            new_start + 2,
            [controller[1], OBJECT_VERSION],
            Some(new1),
            object_iv,
        );

        if real {
            // Program pins were recorded above.  These are the remaining
            // boundary links which an integrated HistoryStep would take from
            // the contract capsule and body aliases.
            for (slot, pair) in [
                (old_start, code_digest),
                (new_start, code_digest),
                (old_start + 1, [current, controller[0]]),
                (old_start + 2, [controller[1], OBJECT_VERSION]),
                (new_start + 1, [next, controller[0]]),
                (new_start + 2, [controller[1], OBJECT_VERSION]),
            ] {
                columns.input_pins.push((IN0, slot, pair[0]));
                columns.input_pins.push((IN0 + 1, slot, pair[1]));
            }
            columns
                .root_pins
                .push((old_start + OLD_OBJECT_SLOTS - 1, [old2[0], old2[1]]));
            columns
                .root_pins
                .push((new_start + NEW_OBJECT_SLOTS - 1, [new2[0], new2[1]]));
            columns.permutations_built_for_real_contracts += CONTRACT_SLOTS;
        }
    }
    columns
}

fn refs() -> SpongeLeafRefs {
    SpongeLeafRefs {
        in_: [IN0, IN0 + 1],
        c: std::array::from_fn(|lane| C0 + lane),
        odd: 1,
        iv: [2, 3],
    }
}

fn terms(alpha: F128) -> Vec<RelationTerm> {
    let mut terms = sponge_leaf_substitution_terms(&refs(), alpha);
    // The generic sponge relation reads IN plainly.  Gate those two terms by
    // the existing wrap-family REGION pattern, as production Meta-A does.
    for term in &mut terms {
        if !term
            .factors
            .iter()
            .any(|factor| matches!(factor, ColRef::Fixed(_)))
        {
            term.factors.insert(0, ColRef::Fixed(0));
        }
    }
    terms
}

fn mle(column: &[F128], point: &[F128]) -> F128 {
    column
        .iter()
        .zip(build_eq_table(point))
        .fold(F128::ZERO, |sum, (value, weight)| sum + *value * weight)
}

fn prove(columns: &MetaColumns, fixed: &[FixedPattern]) -> (DagProof, Duration) {
    let started = Instant::now();
    let committed: Vec<&[F128]> = columns.committed.iter().map(Vec::as_slice).collect();
    let internal: Vec<&[F128]> = columns.s_out.iter().map(Vec::as_slice).collect();
    let mut challenger = FsLaneChallenger::new(b"v2-contract-meta-wrap-v1");

    let beta = challenger.sample_f128();
    let selection_terms = carry_selection_terms(&refs().c, beta);
    let rho = challenger.sample_f128_vec(W_LOG);
    let (selection, selection_point, _) = prove_column_relation(
        F128::ZERO,
        &rho,
        &selection_terms,
        &RelationColumns {
            committed: &committed,
            internal: &internal,
            fixed,
        },
        &mut challenger,
    );
    let mut group_values = [F128::ZERO; 4];
    for (reference, value) in claimed_refs(&selection_terms)
        .iter()
        .zip(selection.final_values.iter())
    {
        if let ColRef::Internal(lane) = reference {
            group_values[*lane] = *value;
        }
    }
    let groups = [LaneClaimGroup {
        point: selection_point,
        values: group_values,
    }];
    let (walk, terminal) = prove_deep_chain_walk(&columns.s0, &groups, &mut challenger);

    let alpha = challenger.sample_f128();
    let substitution_terms = terms(alpha);
    let mut alpha_power = F128::ONE;
    let target = terminal.values.into_iter().fold(F128::ZERO, |sum, value| {
        alpha_power = alpha_power * alpha;
        sum + alpha_power * value
    });
    let (substitution, substitution_point, _) = prove_column_relation(
        target,
        &terminal.point,
        &substitution_terms,
        &RelationColumns {
            committed: &committed,
            internal: &[],
            fixed,
        },
        &mut challenger,
    );
    let mut shifts = Vec::new();
    for (reference, value) in claimed_refs(&substitution_terms)
        .iter()
        .zip(substitution.final_values.iter())
    {
        if let ColRef::CommittedShift(column) | ColRef::CommittedShift2(column) = reference {
            let shift_log = usize::from(matches!(reference, ColRef::CommittedShift2(_)));
            let (proof, _) = prove_shift_discharge_pow2(
                committed[*column],
                &substitution_point,
                *value,
                shift_log,
                &mut challenger,
            );
            shifts.push((shift_log, *column, proof));
        }
    }
    (
        DagProof {
            selection,
            walk,
            substitution,
            shifts,
        },
        started.elapsed(),
    )
}

fn verify_and_pin(
    columns: &MetaColumns,
    fixed: &[FixedPattern],
    proof: &DagProof,
) -> Result<Duration, String> {
    let started = Instant::now();
    let committed: Vec<&[F128]> = columns.committed.iter().map(Vec::as_slice).collect();
    let mut challenger = FsLaneChallenger::new(b"v2-contract-meta-wrap-v1");
    let mut pending: Vec<(usize, Vec<F128>, F128)> = Vec::new();

    let beta = challenger.sample_f128();
    let selection_terms = carry_selection_terms(&refs().c, beta);
    let rho = challenger.sample_f128_vec(W_LOG);
    let selection_point = verify_column_relation(
        W_LOG,
        F128::ZERO,
        &rho,
        &selection_terms,
        fixed,
        &proof.selection,
        &mut challenger,
    )
    .map_err(|error| format!("selection: {error}"))?;
    let mut group_values = [F128::ZERO; 4];
    for (reference, value) in claimed_refs(&selection_terms)
        .iter()
        .zip(proof.selection.final_values.iter())
    {
        match reference {
            ColRef::Committed(column) => pending.push((*column, selection_point.clone(), *value)),
            ColRef::Internal(lane) => group_values[*lane] = *value,
            _ => return Err("unexpected selection ref".to_owned()),
        }
    }
    let groups = [LaneClaimGroup {
        point: selection_point,
        values: group_values,
    }];
    let terminal = verify_deep_chain_walk(W_LOG, &groups, &proof.walk, &mut challenger)
        .map_err(|error| format!("walk: {error}"))?;

    let alpha = challenger.sample_f128();
    let substitution_terms = terms(alpha);
    let mut alpha_power = F128::ONE;
    let target = terminal.values.into_iter().fold(F128::ZERO, |sum, value| {
        alpha_power = alpha_power * alpha;
        sum + alpha_power * value
    });
    let substitution_point = verify_column_relation(
        W_LOG,
        target,
        &terminal.point,
        &substitution_terms,
        fixed,
        &proof.substitution,
        &mut challenger,
    )
    .map_err(|error| format!("substitution: {error}"))?;
    let mut shift_cursor = 0usize;
    for (reference, value) in claimed_refs(&substitution_terms)
        .iter()
        .zip(proof.substitution.final_values.iter())
    {
        match reference {
            ColRef::Committed(column) => {
                pending.push((*column, substitution_point.clone(), *value));
            }
            ColRef::CommittedShift(column) | ColRef::CommittedShift2(column) => {
                let (shift_log, proof_column, shift) = &proof.shifts[shift_cursor];
                shift_cursor += 1;
                if proof_column != column {
                    return Err("shift column order".to_owned());
                }
                let point = verify_shift_discharge_pow2(
                    W_LOG,
                    &substitution_point,
                    *value,
                    *shift_log,
                    shift,
                    &mut challenger,
                )
                .map_err(|error| format!("shift: {error}"))?;
                pending.push((*column, point, shift.final_value));
            }
            _ => return Err("unexpected substitution ref".to_owned()),
        }
    }
    if shift_cursor != proof.shifts.len() {
        return Err("unused shifts".to_owned());
    }
    for (column, point, value) in pending {
        if mle(committed[column], &point) != value {
            return Err(format!("committed opening mismatch on column {column}"));
        }
    }
    for (column, slot, value) in &columns.input_pins {
        if columns.committed[*column][*slot] != *value {
            return Err(format!(
                "input pin mismatch at column {column}, slot {slot}"
            ));
        }
    }
    for (slot, digest) in &columns.root_pins {
        if columns.committed[C0][*slot] != digest[0]
            || columns.committed[C0 + 1][*slot] != digest[1]
        {
            return Err(format!("object root pin mismatch at slot {slot}"));
        }
    }
    Ok(started.elapsed())
}

fn proof_sizes(proof: &DagProof) -> Value {
    let selection = bincode::serialized_size(&proof.selection).unwrap() as usize;
    let walk = bincode::serialized_size(&proof.walk).unwrap() as usize;
    let substitution = bincode::serialized_size(&proof.substitution).unwrap() as usize;
    let shifts: usize = proof
        .shifts
        .iter()
        .map(|(_, _, proof)| bincode::serialized_size(proof).unwrap() as usize)
        .sum();
    json!({
        "selection": selection,
        "walk": walk,
        "substitution": substitution,
        "shifts": shifts,
        "total_component_bytes": selection + walk + substitution + shifts
    })
}

fn boundary_pins_match(columns: &MetaColumns) -> bool {
    columns
        .input_pins
        .iter()
        .all(|(column, slot, value)| columns.committed[*column][*slot] == *value)
        && columns.root_pins.iter().all(|(slot, digest)| {
            columns.committed[C0][*slot] == digest[0]
                && columns.committed[C0 + 1][*slot] == digest[1]
        })
}

fn median(mut values: Vec<u128>) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn run_case(contract: bool) -> (Value, DagProof, MetaColumns) {
    let mut build_micros = Vec::new();
    let mut retained_columns = None;
    for _ in 0..5 {
        let started = Instant::now();
        let columns = build_columns(contract);
        build_micros.push(started.elapsed().as_micros());
        retained_columns = Some(columns);
    }
    let columns = retained_columns.expect("at least one column-build sample");
    let fixed = fixed_patterns(contract);
    let mut prove_micros = Vec::new();
    let mut verify_micros = Vec::new();
    let mut retained = None;
    for _ in 0..3 {
        let (proof, prove_time) = prove(&columns, &fixed);
        let verify_time = verify_and_pin(&columns, &fixed, &proof).expect("honest Meta-A DAG");
        prove_micros.push(prove_time.as_micros());
        verify_micros.push(verify_time.as_micros());
        retained = Some(proof);
    }
    let proof = retained.unwrap();
    let sub_refs = claimed_refs(&terms(F128::ONE));
    let result = json!({
        "kind": if contract { "tx_wrap_plus_contract_commitments" } else { "tx_wrap_only" },
        "w_log": W_LOG,
        "domain_slots": W,
        "fixed_pattern_count": fixed.len(),
        "fixed_low_logs": fixed.iter().map(|pattern| pattern.low_log).collect::<Vec<_>>(),
        "fixed_hi_gate_lengths": fixed.iter().map(|pattern| pattern.hi_gate.as_ref().map_or(0, |(_, bits)| bits.len())).collect::<Vec<_>>(),
        "substitution_terms": terms(F128::ONE).len(),
        "substitution_claimed_refs": sub_refs.len(),
        "substitution_shift_refs": sub_refs.iter().filter(|reference| matches!(reference, ColRef::CommittedShift(_) | ColRef::CommittedShift2(_))).count(),
        "proof_component_sizes": proof_sizes(&proof),
        "column_build_median_micros": median(build_micros),
        "prove_median_micros": median(prove_micros),
        "verify_and_pin_median_micros": median(verify_micros),
        "real_contract_calls": if contract { REAL_CALLS } else { 0 },
        "contract_slots_per_transaction_block": if contract { CONTRACT_SLOTS } else { 0 },
        "unused_tail_slots": if contract { TX_BLOCK_SLOTS - CONTRACT_END } else { TX_BLOCK_SLOTS - CONTRACT_BASE },
        "semantic_input_cell_pins_for_real_calls": columns.input_pins.len(),
        "object_root_pins_for_real_calls": columns.root_pins.len() * 2,
        "permutations_built_for_real_contracts": columns.permutations_built_for_real_contracts,
        "roundtrip_verified": true
    });
    (result, proof, columns)
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
    assert!(args.len() <= 2, "usage: contract_meta_wrap [NEW_JSON_FILE]");

    assert_eq!(META_HALF + TX_TILES * TX_BLOCK_SLOTS, W);
    assert!(CONTRACT_END <= TX_BLOCK_SLOTS);
    let (baseline, baseline_proof, _) = run_case(false);
    let (augmented, augmented_proof, augmented_columns) = run_case(true);

    // A changed committed program cell fails the original capsule boundary
    // equality.  The walk proves what is committed; this separate equality is
    // what ties that committed cell to the contract proof's semantic value.
    let mut corrupted = augmented_columns.clone();
    let (column, slot, _) = corrupted.input_pins[0];
    corrupted.committed[column][slot] += F128::ONE;
    let corrupt_rejected = !boundary_pins_match(&corrupted);
    assert!(corrupt_rejected);

    let base_size = proof_sizes(&baseline_proof)["total_component_bytes"]
        .as_u64()
        .unwrap();
    let augmented_size = proof_sizes(&augmented_proof)["total_component_bytes"]
        .as_u64()
        .unwrap();
    assert_eq!(base_size, augmented_size);

    let result = json!({
        "schema": 1,
        "unix_time": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "source_revision": String::from_utf8(
            std::process::Command::new("git").args(["rev-parse", "HEAD"]).output().unwrap().stdout
        ).unwrap().trim(),
        "kind": "b25_contract_commitments_inside_existing_meta_a_wrap_family",
        "layout": {
            "meta_a_domain": "2^12 slots, unchanged",
            "per_transaction_block": "64 slots",
            "body_tree": "0..32",
            "tx8x2_wrap": 32,
            "contract_code_commitment": "33..41 (8 permutations for 16 program fields)",
            "current_object_commitment": "41..44 (code digest, State, controller, version)",
            "next_object_commitment": "44..47 (code digest, State, controller, version)",
            "remaining_padding": "47..64"
        },
        "baseline": baseline,
        "augmented": augmented,
        "proof_component_bytes_unchanged": base_size == augmented_size,
        "recursive_relation_shape_unchanged": true,
        "corrupted_program_cell_rejected_by_boundary_pin": corrupt_rejected,
        "established": [
            "the contract code and old/new object commitment schedule fits in the existing 31-slot tail of every B25 Meta-A transaction block",
            "extending the existing Tx8x2 wrap sponge patterns keeps the same four fixed tables, their low-log and high-gate arity, the same eight substitution terms, the same claimed refs and the same shift layout",
            "the Meta-A walk domain remains 2^12 and the measured standalone proof component byte count is identical",
            "four real calls require no additional deep-chain walk and their program/object hashes round-trip through the actual Poseidon2b walk",
            "a corrupted program cell is rejected by the original semantic boundary pin"
        ],
        "not_established": [
            "the complete production Meta-A union after changing its canonical wrap tables",
            "the recursive cell-pin links to the capsule bank and Tx body",
            "a final contract context/effect ABI or integrated HistoryStep timing",
            "a new end-to-end soundness certificate"
        ]
    });
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    if let Some(path) = args.get(1) {
        save_new(path, &result);
    }
}
