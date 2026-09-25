//! Research-only recursive row model for a composite wallet + policy capsule.
//!
//! The current 66-round Poseidon owner relation remains intact in rows 0..66.
//! A power-of-two policy trace starts at row 96 and ends before row 128. This
//! harness measures only the incremental recursive algebra after exposing
//! aliases which the current terminal and post-claim traces already compute:
//! the 128-way row equality tensor, lane basis, bank-high selectors, and the
//! current owner terminal's active selector and inner expression. Owner and
//! policy share the same five terminal operand claims.
//! It is not a HistoryStep integration or a soundness result.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use noid_core::{Block128, Block256, TowerField};
use noid_gkr::zk_auth_capsule::mle_weights_low_to_high;
use noid_recursive::acceptance::trace::{
    alloc_block, alloc_block256, constrain_nonzero_ext, eq_ind_partial_eval_ext_trace, mul_ext,
    mul_ext_base, ExtExpr, FieldR1csBuilder, LinExpr, F256,
};
use serde_json::{json, Value};

const BANK_VARS: usize = 11;
const LANE_BITS: usize = 2;
const ROW_BITS: usize = 7;
const ROW_START: usize = LANE_BITS;
const ROW_END: usize = ROW_START + ROW_BITS;
const POLICY_BASE_ROW: usize = 96;
const AUTH_TILES_B25: usize = 32;
const BODY_TILES_B25: usize = 25;
const AUTH_TILES_B255: usize = 256;
const BODY_TILES_B255: usize = 255;
const B25_USEFUL_ROWS: usize = 4_185_273;
const B25_PADDED_ROWS: usize = 1 << 22;
const B255_USEFUL_ROWS: usize = 16_360_489;
const B255_PADDED_ROWS: usize = 1 << 24;
const CURRENT_TERMINAL_ROWS: usize = 493;
const CURRENT_POST_CLAIM_ROWS: usize = 893;
const CURRENT_COMPOSITION_ROWS: usize = 1_852;

fn elem(index: usize, domain: u128) -> Block128 {
    let value = domain
        .wrapping_mul(index as u128 + 1)
        .rotate_left(((13 * index + 7) % 127) as u32)
        ^ (index as u128 + 23).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let value = Block128::from(value);
    if value == Block128::ZERO || value == Block128::ONE {
        value + Block128::from(2u128)
    } else {
        value
    }
}

fn wide(index: usize, domain: u128) -> Block256 {
    Block256::from_raw_challenge_lanes(
        elem(2 * index, domain),
        elem(2 * index + 1, domain ^ 0xC1_0256),
    )
}

fn product(b: &mut FieldR1csBuilder, factors: &[ExtExpr]) -> ExtExpr {
    assert!(!factors.is_empty());
    let mut result = factors[0].clone();
    for factor in &factors[1..] {
        result = mul_ext(b, &result, factor);
    }
    result
}

fn lane_basis(b: &mut FieldR1csBuilder, point: &[ExtExpr; BANK_VARS]) -> [ExtExpr; 4] {
    let cross = mul_ext(b, &point[0], &point[1]);
    [
        ExtExpr::one().add(&point[0]).add(&point[1]).add(&cross),
        point[0].add(&cross),
        point[1].add(&cross),
        cross,
    ]
}

fn bank_high_zero(b: &mut FieldR1csBuilder, point: &[ExtExpr; BANK_VARS]) -> ExtExpr {
    mul_ext(
        b,
        &point[ROW_END].add_const(F256::ONE),
        &point[ROW_END + 1].add_const(F256::ONE),
    )
}

fn unified_policy_terminal(
    b: &mut FieldR1csBuilder,
    owner_active: &ExtExpr,
    owner_inner: &ExtExpr,
    lanes: &[ExtExpr; 4],
    program: &[Vec<LinExpr>],
    row_eq: &[ExtExpr],
    lane_at_point: &[ExtExpr; 4],
    bank_high: &ExtExpr,
) -> ExtExpr {
    let slots = program.len();
    let coefficients = program[0].len();
    assert!(slots.is_power_of_two());
    assert!(coefficients == 2 || coefficients == 4);
    assert_eq!(row_eq.len(), 128);
    assert!(POLICY_BASE_ROW % slots == 0);
    assert!(POLICY_BASE_ROW + slots < 128);

    let row_active = row_eq[POLICY_BASE_ROW..POLICY_BASE_ROW + slots]
        .iter()
        .fold(ExtExpr::zero(), |sum, value| sum.add(value));
    let policy_active = mul_ext(b, bank_high, &row_active);
    let code_lane = mul_ext(b, bank_high, &lane_at_point[0]);

    let code_at_point: Vec<ExtExpr> = (0..coefficients)
        .map(|coefficient| {
            (0..slots).fold(ExtExpr::zero(), |sum, slot| {
                sum.add(&mul_ext_base(
                    b,
                    &row_eq[POLICY_BASE_ROW + slot],
                    &program[slot][coefficient],
                ))
            })
        })
        .collect();
    let code_table: Vec<ExtExpr> = code_at_point
        .iter()
        .map(|value| mul_ext(b, &code_lane, value))
        .collect();

    let identity: [ExtExpr; 3] =
        std::array::from_fn(|lane| mul_ext(b, &policy_active, &lane_at_point[lane + 1]));
    let mut policy_inner = code_table[0].clone();
    policy_inner = policy_inner.add(&mul_ext(b, &code_table[1], &lanes[0]));
    if coefficients == 4 {
        let context_coefficient = code_table[2].add(&identity[0]);
        policy_inner = policy_inner.add(&mul_ext(b, &context_coefficient, &lanes[1]));
        policy_inner = policy_inner.add(&mul_ext(b, &identity[1], &lanes[2]));
        policy_inner = policy_inner.add(&mul_ext(b, &identity[2], &lanes[3]));
        let cross = mul_ext(b, &lanes[0], &lanes[1]);
        policy_inner = policy_inner.add(&mul_ext(b, &code_table[3], &cross));
    } else {
        for lane in 0..3 {
            policy_inner = policy_inner.add(&mul_ext(b, &identity[lane], &lanes[lane + 1]));
        }
    }
    mul_ext(
        b,
        &owner_active.add(&policy_active),
        &owner_inner.add(&policy_inner),
    )
}

fn native_unified_policy_terminal(
    point: &[Block256; BANK_VARS],
    claims: &[Block256; 5],
    program: &[Vec<Block128>],
    owner_active: Block256,
    owner_inner: Block256,
) -> Block256 {
    let slots = program.len();
    let coefficients = program[0].len();
    let eq = mle_weights_low_to_high(point);
    let active = (POLICY_BASE_ROW..POLICY_BASE_ROW + slots).fold(Block256::ZERO, |sum, row| {
        sum + (0..4).fold(Block256::ZERO, |lane_sum, lane| {
            lane_sum + eq[4 * row + lane]
        })
    });
    let mut code_at_point = vec![Block256::ZERO; coefficients];
    for slot in 0..slots {
        let table_weight = eq[4 * (POLICY_BASE_ROW + slot)];
        for coefficient in 0..coefficients {
            code_at_point[coefficient] += table_weight * Block256::from(program[slot][coefficient]);
        }
    }
    let identity: [Block256; 3] = std::array::from_fn(|lane| {
        (POLICY_BASE_ROW..POLICY_BASE_ROW + slots)
            .fold(Block256::ZERO, |sum, row| sum + eq[4 * row + lane + 1])
    });
    let mut policy_inner = code_at_point[0];
    policy_inner += code_at_point[1] * claims[1];
    if coefficients == 4 {
        policy_inner += (code_at_point[2] + identity[0]) * claims[2];
        policy_inner += identity[1] * claims[3];
        policy_inner += identity[2] * claims[4];
        policy_inner += code_at_point[3] * claims[1] * claims[2];
    } else {
        for lane in 0..3 {
            policy_inner += identity[lane] * claims[lane + 2];
        }
    }
    (owner_active + active) * (owner_inner + policy_inner)
}

fn selector_for_fixed_bits(
    b: &mut FieldR1csBuilder,
    point: &[ExtExpr; BANK_VARS],
    first_row_bit: usize,
    row_value: usize,
) -> ExtExpr {
    let factors: Vec<_> = (first_row_bit..ROW_BITS)
        .map(|bit| {
            let coordinate = &point[ROW_START + bit];
            if ((row_value >> bit) & 1) == 1 {
                coordinate.clone()
            } else {
                coordinate.add_const(F256::ONE)
            }
        })
        .collect();
    product(b, &factors)
}

fn low_zero(b: &mut FieldR1csBuilder, point: &[ExtExpr], bits: usize) -> ExtExpr {
    let factors: Vec<_> = point[..bits]
        .iter()
        .map(|coordinate| coordinate.add_const(F256::ONE))
        .collect();
    product(b, &factors)
}

fn low_one(b: &mut FieldR1csBuilder, point: &[ExtExpr], bits: usize) -> ExtExpr {
    product(b, &point[..bits])
}

fn same_low(b: &mut FieldR1csBuilder, r: &[ExtExpr], s: &[ExtExpr], bits: usize) -> ExtExpr {
    let factors: Vec<_> = (0..bits)
        .map(|bit| r[bit].add(&s[bit]).add_const(F256::ONE))
        .collect();
    product(b, &factors)
}

fn increment_low_without_overflow(
    b: &mut FieldR1csBuilder,
    r: &[ExtExpr],
    s: &[ExtExpr],
    bits: usize,
) -> ExtExpr {
    let a: Vec<_> = (0..bits)
        .map(|bit| mul_ext(b, &r[bit].add_const(F256::ONE), &s[bit]))
        .collect();
    let carry: Vec<_> = (0..bits)
        .map(|bit| mul_ext(b, &r[bit], &s[bit].add_const(F256::ONE)))
        .collect();
    let eq: Vec<_> = (0..bits)
        .map(|bit| r[bit].add(&s[bit]).add_const(F256::ONE))
        .collect();
    let mut suffix = vec![ExtExpr::zero(); bits];
    suffix[bits - 1] = eq[bits - 1].clone();
    for bit in (1..bits - 1).rev() {
        suffix[bit] = mul_ext(b, &eq[bit], &suffix[bit + 1]);
    }
    let mut result = a[bits - 1].clone();
    for bit in (0..bits - 1).rev() {
        result = mul_ext(b, &a[bit], &suffix[bit + 1]).add(&mul_ext(b, &carry[bit], &result));
    }
    result
}

/// Additional policy operand and boundary functionals. The current Poseidon
/// functionals and the five existing dummy pads remain untouched. The caller
/// supplies lane and bank-high aliases already constructed by the current
/// post-claim trace, so this is an integration delta rather than a standalone
/// duplicate.
struct PolicyPostDeltaOutput {
    functionals: [ExtExpr; 13],
    scalar_rlc_sink: ExtExpr,
    relation_rlc_sink: ExtExpr,
}

fn policy_post_claim_delta(
    b: &mut FieldR1csBuilder,
    r: &[ExtExpr; BANK_VARS],
    s: &[ExtExpr; BANK_VARS],
    lane_at_s: &[ExtExpr; 4],
    r_bank_high: &ExtExpr,
    s_bank_high: &ExtExpr,
    slots: usize,
    eta: &ExtExpr,
) -> PolicyPostDeltaOutput {
    let low_bits = slots.trailing_zeros() as usize;
    assert_eq!(1usize << low_bits, slots);
    assert!(POLICY_BASE_ROW % slots == 0);
    assert!(POLICY_BASE_ROW + slots < 128);

    let r_row_prefix = selector_for_fixed_bits(b, r, low_bits, POLICY_BASE_ROW);
    let s_row_prefix = selector_for_fixed_bits(b, s, low_bits, POLICY_BASE_ROW);
    let r_region = mul_ext(b, r_bank_high, &r_row_prefix);
    let s_region = mul_ext(b, s_bank_high, &s_row_prefix);
    let joint_region = mul_ext(b, &r_region, &s_region);

    let same = same_low(
        b,
        &r[ROW_START..ROW_START + low_bits],
        &s[ROW_START..ROW_START + low_bits],
        low_bits,
    );
    let same_region = mul_ext(b, &joint_region, &same);
    let lane_relations: [ExtExpr; 4] =
        std::array::from_fn(|lane| mul_ext(b, &same_region, &lane_at_s[lane]));

    let lane_equal = mul_ext(
        b,
        &r[0].add(&s[0]).add_const(F256::ONE),
        &r[1].add(&s[1]).add_const(F256::ONE),
    );

    let normal_shift = increment_low_without_overflow(
        b,
        &r[ROW_START..ROW_START + low_bits],
        &s[ROW_START..ROW_START + low_bits],
        low_bits,
    );
    let normal_shift = mul_ext(b, &joint_region, &normal_shift);
    let normal_shift = mul_ext(b, &normal_shift, &lane_equal);

    let final_row = POLICY_BASE_ROW + slots;
    let s_final_prefix = selector_for_fixed_bits(b, s, low_bits, final_row);
    let s_final_region = mul_ext(b, s_bank_high, &s_final_prefix);
    let r_low_one = low_one(b, &r[ROW_START..], low_bits);
    let s_low_zero = low_zero(b, &s[ROW_START..], low_bits);
    let overflow_shift = product(
        b,
        &[
            r_region.clone(),
            s_final_region.clone(),
            r_low_one,
            s_low_zero.clone(),
            lane_equal,
        ],
    );
    let increment = normal_shift.add(&overflow_shift);

    let initial_row = mul_ext(b, &s_region, &s_low_zero);
    let final_row = mul_ext(b, &s_final_region, &s_low_zero);
    let initial_boundary: [ExtExpr; 4] =
        std::array::from_fn(|lane| mul_ext(b, &initial_row, &lane_at_s[lane]));
    let final_boundary: [ExtExpr; 4] =
        std::array::from_fn(|lane| mul_ext(b, &final_row, &lane_at_s[lane]));

    // Eight additional scalar claims and eight additional relation
    // functionals each extend the existing eta Horner fold by eight products.
    let mut scalar_extension = ExtExpr::zero();
    let mut relation_extension = ExtExpr::zero();
    for value in initial_boundary.iter().chain(&final_boundary) {
        scalar_extension = value.add(&mul_ext(b, eta, &scalar_extension));
        relation_extension = value.add(&mul_ext(b, eta, &relation_extension));
    }

    let functionals = [
        increment,
        lane_relations[0].clone(),
        lane_relations[1].clone(),
        lane_relations[2].clone(),
        lane_relations[3].clone(),
        initial_boundary[0].clone(),
        initial_boundary[1].clone(),
        initial_boundary[2].clone(),
        initial_boundary[3].clone(),
        final_boundary[0].clone(),
        final_boundary[1].clone(),
        final_boundary[2].clone(),
        final_boundary[3].clone(),
    ];
    PolicyPostDeltaOutput {
        functionals,
        scalar_rlc_sink: scalar_extension,
        relation_rlc_sink: relation_extension,
    }
}

fn native_policy_post_functionals(
    r: &[Block256; BANK_VARS],
    s: &[Block256; BANK_VARS],
    slots: usize,
) -> [Block256; 13] {
    let eq_r = mle_weights_low_to_high(r);
    let eq_s = mle_weights_low_to_high(s);
    let mut operand_weights: [Vec<Block256>; 5] =
        std::array::from_fn(|_| vec![Block256::ZERO; 1 << BANK_VARS]);
    for row in POLICY_BASE_ROW..POLICY_BASE_ROW + slots {
        for output_lane in 0..4 {
            let relation_index = 4 * row + output_lane;
            let weight = eq_r[relation_index];
            operand_weights[0][4 * (row + 1) + output_lane] += weight;
            for input_lane in 0..4 {
                operand_weights[1 + input_lane][4 * row + input_lane] += weight;
            }
        }
    }
    let operand_values: [Block256; 5] = std::array::from_fn(|operand| {
        operand_weights[operand]
            .iter()
            .zip(&eq_s)
            .fold(Block256::ZERO, |sum, (weight, basis)| {
                sum + *weight * *basis
            })
    });
    let initial: [Block256; 4] = std::array::from_fn(|lane| eq_s[4 * POLICY_BASE_ROW + lane]);
    let final_state: [Block256; 4] =
        std::array::from_fn(|lane| eq_s[4 * (POLICY_BASE_ROW + slots) + lane]);
    [
        operand_values[0],
        operand_values[1],
        operand_values[2],
        operand_values[3],
        operand_values[4],
        initial[0],
        initial[1],
        initial[2],
        initial[3],
        final_state[0],
        final_state[1],
        final_state[2],
        final_state[3],
    ]
}

fn next_power_of_two(value: usize) -> usize {
    value.next_power_of_two()
}

fn run_case(slots: usize, coefficients: usize) -> Value {
    assert!(matches!(slots, 8 | 16));
    assert!(matches!(coefficients, 2 | 4));

    let rho_native: [Block256; BANK_VARS] =
        std::array::from_fn(|index| wide(index, 0x5248_4F00 + slots as u128));
    let r_native: [Block256; BANK_VARS] =
        std::array::from_fn(|index| wide(index + 32, 0x5250_4F00 + slots as u128));
    let s_native: [Block256; BANK_VARS] =
        std::array::from_fn(|index| wide(index + 64, 0x5350_4F00 + slots as u128));
    let claims_native: [Block256; 5] =
        std::array::from_fn(|index| wide(index + 96, 0x434C_4100 + slots as u128));
    let eta_native = wide(127, 0x4554_4100 + coefficients as u128);

    let mut b = FieldR1csBuilder::new();
    let rho: [ExtExpr; BANK_VARS] =
        std::array::from_fn(|index| alloc_block256(&mut b, rho_native[index]));
    let r: [ExtExpr; BANK_VARS] =
        std::array::from_fn(|index| alloc_block256(&mut b, r_native[index]));
    let s: [ExtExpr; BANK_VARS] =
        std::array::from_fn(|index| alloc_block256(&mut b, s_native[index]));
    let lanes: [ExtExpr; 4] =
        std::array::from_fn(|lane| alloc_block256(&mut b, claims_native[lane + 1]));
    let owner_active_native = wide(120, 0x4F57_4E41 + slots as u128);
    let owner_inner_native = claims_native[0];
    let owner_active = alloc_block256(&mut b, owner_active_native);
    let owner_inner = alloc_block256(&mut b, owner_inner_native);
    let eta = alloc_block256(&mut b, eta_native);
    constrain_nonzero_ext(&mut b, &eta);
    let program_native: Vec<Vec<Block128>> = (0..slots)
        .map(|slot| {
            (0..coefficients)
                .map(|coefficient| elem(slot * coefficients + coefficient, 0x5052_4F47))
                .collect()
        })
        .collect();
    let program: Vec<Vec<LinExpr>> = program_native
        .iter()
        .map(|slot| {
            slot.iter()
                .map(|value| alloc_block(&mut b, *value))
                .collect()
        })
        .collect();

    // These three objects already exist inside the current recursive traces.
    // Building them before the measured interval models a zero-row exposure
    // refactor, not free recomputation in the final circuit.
    let row_eq = eq_ind_partial_eval_ext_trace(&mut b, &r[ROW_START..ROW_END]);
    let lane_at_r = lane_basis(&mut b, &r);
    let r_bank_high = bank_high_zero(&mut b, &r);
    let lane_at_s = lane_basis(&mut b, &s);
    let s_bank_high = bank_high_zero(&mut b, &s);
    let _rho_alias = &rho;

    let terminal_start = b.num_wires();
    let terminal = unified_policy_terminal(
        &mut b,
        &owner_active,
        &owner_inner,
        &lanes,
        &program,
        &row_eq,
        &lane_at_r,
        &r_bank_high,
    );
    let terminal_gross_rows = b.num_wires() - terminal_start;
    let terminal_replaced_rows = 3;
    let terminal_rows = terminal_gross_rows - terminal_replaced_rows;
    let terminal_expected = native_unified_policy_terminal(
        &r_native,
        &claims_native,
        &program_native,
        owner_active_native,
        owner_inner_native,
    );
    assert_eq!(
        terminal.eval(b.values()),
        F256::from_tower(terminal_expected)
    );

    let post_start = b.num_wires();
    let post = policy_post_claim_delta(
        &mut b,
        &r,
        &s,
        &lane_at_s,
        &r_bank_high,
        &s_bank_high,
        slots,
        &eta,
    );
    let post_rows = b.num_wires() - post_start;
    let post_expected = native_policy_post_functionals(&r_native, &s_native, slots);
    for (index, (actual, expected)) in post.functionals.iter().zip(post_expected).enumerate() {
        assert_eq!(
            actual.eval(b.values()),
            F256::from_tower(expected),
            "policy post functional {index}"
        );
    }

    // Keep every result live in the generated relation without adding rows.
    let _sink = post
        .functionals
        .iter()
        .fold(terminal, |sum, value| sum.add(value))
        .add(&post.scalar_rlc_sink)
        .add(&post.relation_rlc_sink);
    let (r1cs, witness) = b.build();
    assert!(r1cs.satisfies(&witness));

    let per_tile_delta = terminal_rows + post_rows;
    let projected_useful = B25_USEFUL_ROWS + AUTH_TILES_B25 * per_tile_delta;
    let projected_b25_body_only = B25_USEFUL_ROWS + BODY_TILES_B25 * per_tile_delta;
    let projected_b255_all = B255_USEFUL_ROWS + AUTH_TILES_B255 * per_tile_delta;
    let projected_b255_body_only = B255_USEFUL_ROWS + BODY_TILES_B255 * per_tile_delta;
    json!({
        "slots": slots,
        "coefficients_per_slot": coefficients,
        "program_public_bytes": slots * coefficients * 16,
        "policy_state_start_row": POLICY_BASE_ROW,
        "policy_state_final_row": POLICY_BASE_ROW + slots,
        "unified_terminal_gross_rows": terminal_gross_rows,
        "existing_owner_final_multiply_rows_replaced": terminal_replaced_rows,
        "incremental_unified_terminal_rows": terminal_rows,
        "incremental_post_claim_rows_including_eight_claim_rlc_extensions": post_rows,
        "incremental_rows_per_authorization_tile": per_tile_delta,
        "current_owner_terminal_rows": CURRENT_TERMINAL_ROWS,
        "projected_composite_owner_terminal_rows": CURRENT_TERMINAL_ROWS + terminal_rows,
        "current_post_claim_rows": CURRENT_POST_CLAIM_ROWS,
        "projected_composite_post_claim_rows": CURRENT_POST_CLAIM_ROWS + post_rows,
        "current_authorization_composition_rows": CURRENT_COMPOSITION_ROWS,
        "projected_authorization_composition_rows": CURRENT_COMPOSITION_ROWS + per_tile_delta,
        "b25_authorization_tiles": AUTH_TILES_B25,
        "b25_current_useful_rows": B25_USEFUL_ROWS,
        "b25_current_padded_rows": B25_PADDED_ROWS,
        "b25_projected_useful_rows_before_other_contract_machinery": projected_useful,
        "b25_projected_padded_rows": next_power_of_two(projected_useful),
        "b25_crosses_m22_before_other_contract_machinery": projected_useful > B25_PADDED_ROWS,
        "rows_over_m22": projected_useful.saturating_sub(B25_PADDED_ROWS),
        "b25_body_tiles": BODY_TILES_B25,
        "b25_projected_useful_rows_if_only_body_tiles_are_contract_capable": projected_b25_body_only,
        "b25_rows_left_if_only_body_tiles_are_contract_capable": B25_PADDED_ROWS.saturating_sub(projected_b25_body_only),
        "b25_body_only_crosses_m22": projected_b25_body_only > B25_PADDED_ROWS,
        "b255_authorization_tiles": AUTH_TILES_B255,
        "b255_body_tiles": BODY_TILES_B255,
        "b255_current_useful_rows": B255_USEFUL_ROWS,
        "b255_current_padded_rows": B255_PADDED_ROWS,
        "b255_projected_useful_rows_all_authorization_tiles": projected_b255_all,
        "b255_rows_left_all_authorization_tiles": B255_PADDED_ROWS.saturating_sub(projected_b255_all),
        "b255_projected_useful_rows_body_tiles_only": projected_b255_body_only,
        "b255_rows_left_body_tiles_only": B255_PADDED_ROWS.saturating_sub(projected_b255_body_only),
        "measurement_assumes_reuse_of_existing_row_lane_bank_high_owner_active_and_owner_inner_aliases": true,
        "dense_2048_cell_differential_passed": true,
        "r1cs_satisfied": true,
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
        "usage: composite_policy_rows [NEW_JSON_FILE]"
    );
    let cases = [
        run_case(8, 2),
        run_case(8, 4),
        run_case(16, 2),
        run_case(16, 4),
    ];
    let result = json!({
        "schema": 1,
        "unix_time": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "source_revision": String::from_utf8(
            std::process::Command::new("git").args(["rev-parse", "HEAD"]).output().unwrap().stdout
        ).unwrap().trim(),
        "kind": "unified_owner_plus_algebra_native_policy_recursive_row_model",
        "cases": cases,
        "established": [
            "current 66-round private owner relation can remain before a policy trace in the same 128-row State table",
            "dynamic algebra-native program coefficients have a fixed recursive relation",
            "owner and policy reuse the same five terminal operand claims",
            "incremental recursive terminal and post-claim row counts for 8 and 16 slots",
            "all measured R1CS witnesses satisfy their generated matrices",
            "B25 shape projections distinguish all 32 authorization tiles from the 25 body-bearing tiles",
            "B255 shape projections distinguish all 256 authorization tiles from the 255 body-bearing tiles"
        ],
        "not_established": [
            "integrated HistoryStep matrix",
            "program-code commitment or Tx8x2 body binding",
            "object State layout and exact effects",
            "integer semantics, branches, memory, calls or concurrency",
            "native composite PCS proof",
            "soundness accounting for the composite relation",
            "production timing after an m22 or m23 integration"
        ]
    });
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    if let Some(path) = args.get(1) {
        save_new(path, &result);
    }
}
