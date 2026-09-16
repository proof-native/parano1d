//! Research-only recursive row model for a universal eight-step contract
//! capsule. Program opcodes and operands are committed witness cells, not
//! verifier-selected coefficient tables. The model measures the incremental
//! terminal and post-claim algebra after reusing aliases already built by the
//! current authorization verifier.
//!
//! This is deliberately not a production HistoryStep or an ABI. It answers
//! whether exact, non-whitelisted program binding has a credible B25 row
//! budget before that integration is attempted.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use noid_core::{Block128, Block256, TowerField};
use noid_gkr::zk_auth_capsule::mle_weights_low_to_high;
use noid_recursive::acceptance::trace::{
    alloc_block256, constrain_nonzero_ext, eq_ind_partial_eval_ext_trace, mul_ext, ExtExpr,
    FieldR1csBuilder, F256,
};
use serde_json::{json, Value};

const BANK_VARS: usize = 11;
const LANE_BITS: usize = 2;
const ROW_BITS: usize = 7;
const ROW_START: usize = LANE_BITS;
const ROW_END: usize = ROW_START + ROW_BITS;
const POLICY_BASE_ROW: usize = 96;
const PROGRAM_STEPS: usize = 8;
const FINAL_ROW: usize = POLICY_BASE_ROW + PROGRAM_STEPS;
const PROGRAM_BOUNDARY_CLAIMS: usize = PROGRAM_STEPS * 2;

const B25_USEFUL_ROWS: usize = 4_185_273;
const B25_PADDED_ROWS: usize = 1 << 22;
const B25_BODY_SLOTS: usize = 25;
const B25_CANDIDATE_CONTRACT_SLOTS: usize = 4;
const B255_USEFUL_ROWS: usize = 16_360_489;
const B255_PADDED_ROWS: usize = 1 << 24;
const B255_BODY_SLOTS: usize = 255;
const B255_CANDIDATE_CONTRACT_SLOTS: usize = 32;

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

fn small(value: u128) -> F256 {
    F256::from_tower(Block256::from(Block128::from(value)))
}

fn native_small(value: u128) -> Block256 {
    Block256::from(Block128::from(value))
}

fn lagrange4_expr(b: &mut FieldR1csBuilder, opcode: &ExtExpr) -> ([ExtExpr; 4], ExtExpr) {
    let factors: [ExtExpr; 4] = std::array::from_fn(|index| opcode.add_const(small(index as u128)));
    let pair01 = mul_ext(b, &factors[0], &factors[1]);
    let pair23 = mul_ext(b, &factors[2], &factors[3]);
    let numerators = [
        mul_ext(b, &factors[1], &pair23),
        mul_ext(b, &factors[0], &pair23),
        mul_ext(b, &pair01, &factors[3]),
        mul_ext(b, &pair01, &factors[2]),
    ];
    let selectors = std::array::from_fn(|selected| {
        let selected_value = Block128::from(selected as u128);
        let denominator = (0..4)
            .filter(|other| *other != selected)
            .fold(Block128::ONE, |acc, other| {
                acc * (selected_value + Block128::from(other as u128))
            });
        numerators[selected].scale_ext(F256::from_tower(Block256::from(denominator.invert())))
    });
    let validity = mul_ext(b, &pair01, &pair23);
    (selectors, validity)
}

fn lagrange4_native(opcode: Block256) -> ([Block256; 4], Block256) {
    let factors: [Block256; 4] = std::array::from_fn(|index| opcode + native_small(index as u128));
    let pair01 = factors[0] * factors[1];
    let pair23 = factors[2] * factors[3];
    let numerators = [
        factors[1] * pair23,
        factors[0] * pair23,
        pair01 * factors[3],
        pair01 * factors[2],
    ];
    let selectors = std::array::from_fn(|selected| {
        let selected_value = Block128::from(selected as u128);
        let denominator = (0..4)
            .filter(|other| *other != selected)
            .fold(Block128::ONE, |acc, other| {
                acc * (selected_value + Block128::from(other as u128))
            });
        numerators[selected] * Block256::from(denominator.invert())
    });
    (selectors, pair01 * pair23)
}

/// Five shared terminal operands: next-cell relation value, then the four
/// current row lanes (State, context, opcode, operand).
fn universal_contract_terminal(
    b: &mut FieldR1csBuilder,
    operands: &[ExtExpr; 5],
    row_eq: &[ExtExpr],
    lane_at_point: &[ExtExpr; 4],
    bank_high: &ExtExpr,
) -> ExtExpr {
    let row_active = row_eq[POLICY_BASE_ROW..POLICY_BASE_ROW + PROGRAM_STEPS]
        .iter()
        .fold(ExtExpr::zero(), |sum, value| sum.add(value));
    let region = mul_ext(b, bank_high, &row_active);
    let active_state = mul_ext(b, &region, &lane_at_point[0]);
    let active_context = mul_ext(b, &region, &lane_at_point[1]);
    let active_opcode = mul_ext(b, &region, &lane_at_point[2]);

    let state = &operands[1];
    let context = &operands[2];
    let opcode = &operands[3];
    let immediate = &operands[4];
    let (selectors, opcode_validity) = lagrange4_expr(b, opcode);

    let state_times_immediate = mul_ext(b, state, immediate);
    let context_select = state.add(&mul_ext(b, context, &state.add(immediate)));
    let transition = mul_ext(b, &selectors[0], state)
        .add(&mul_ext(b, &selectors[1], &state.add(immediate)))
        .add(&mul_ext(b, &selectors[2], &state_times_immediate))
        .add(&mul_ext(b, &selectors[3], &context_select));

    mul_ext(b, &active_state, &operands[0].add(&transition))
        .add(&mul_ext(b, &active_context, &operands[0].add(context)))
        .add(&mul_ext(b, &active_opcode, &opcode_validity))
}

fn universal_contract_terminal_native(
    point: &[Block256; BANK_VARS],
    operands: &[Block256; 5],
) -> Block256 {
    let eq = mle_weights_low_to_high(point);
    let lane_active: [Block256; 3] = std::array::from_fn(|lane| {
        (POLICY_BASE_ROW..POLICY_BASE_ROW + PROGRAM_STEPS)
            .fold(Block256::ZERO, |sum, row| sum + eq[4 * row + lane])
    });
    let state = operands[1];
    let context = operands[2];
    let opcode = operands[3];
    let immediate = operands[4];
    let (selectors, validity) = lagrange4_native(opcode);
    let transition = selectors[0] * state
        + selectors[1] * (state + immediate)
        + selectors[2] * state * immediate
        + selectors[3] * (state + context * (state + immediate));
    lane_active[0] * (operands[0] + transition)
        + lane_active[1] * (operands[0] + context)
        + lane_active[2] * validity
}

struct PostOutput {
    functionals: Vec<ExtExpr>,
    scalar_rlc_sink: ExtExpr,
    relation_rlc_sink: ExtExpr,
}

fn universal_contract_post_delta(
    b: &mut FieldR1csBuilder,
    r: &[ExtExpr; BANK_VARS],
    s: &[ExtExpr; BANK_VARS],
    lane_at_r: &[ExtExpr; 4],
    lane_at_s: &[ExtExpr; 4],
    r_bank_high: &ExtExpr,
    s_bank_high: &ExtExpr,
    eta: &ExtExpr,
    bind_context_vector: bool,
) -> PostOutput {
    let low_bits = PROGRAM_STEPS.trailing_zeros() as usize;
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
    let active_output_lane = lane_at_r[0].add(&lane_at_r[1]).add(&lane_at_r[2]);
    let current_same = mul_ext(b, &joint_region, &same);
    let current_base = mul_ext(b, &current_same, &active_output_lane);
    let current_operands: [ExtExpr; 4] =
        std::array::from_fn(|lane| mul_ext(b, &current_base, &lane_at_s[lane]));

    // Exact multilinear extension of equality restricted to output lanes 0
    // and 1. Multiplying the all-lane equality polynomial by `(1 + r_1)`
    // would agree only on the Boolean cube and would not be the functional
    // used by the post-claim protocol at random r/s.
    let lane_equal_zero_one =
        mul_ext(b, &lane_at_r[0], &lane_at_s[0]).add(&mul_ext(b, &lane_at_r[1], &lane_at_s[1]));
    let normal_shift = increment_low_without_overflow(
        b,
        &r[ROW_START..ROW_START + low_bits],
        &s[ROW_START..ROW_START + low_bits],
        low_bits,
    );
    let normal_increment = product(
        b,
        &[
            joint_region.clone(),
            normal_shift,
            lane_equal_zero_one.clone(),
        ],
    );

    let s_final_prefix = selector_for_fixed_bits(b, s, low_bits, FINAL_ROW);
    let s_final_region = mul_ext(b, s_bank_high, &s_final_prefix);
    let r_low_one = low_one(b, &r[ROW_START..], low_bits);
    let s_low_zero = low_zero(b, &s[ROW_START..], low_bits);
    let overflow_increment = product(
        b,
        &[
            r_region.clone(),
            s_final_region.clone(),
            r_low_one,
            s_low_zero.clone(),
            lane_equal_zero_one,
        ],
    );
    let increment = normal_increment.add(&overflow_increment);

    // Exact program binding: every opcode and operand cell is a public sparse
    // boundary claim. Low-row equality weights share one 3-bit tensor.
    let low_row_eq = eq_ind_partial_eval_ext_trace(b, &s[ROW_START..ROW_START + low_bits]);
    let row_weights: Vec<_> = low_row_eq
        .iter()
        .map(|weight| mul_ext(b, &s_region, weight))
        .collect();
    let context_claims = if bind_context_vector {
        PROGRAM_STEPS
    } else {
        1
    };
    let boundary_claims = PROGRAM_BOUNDARY_CLAIMS + 2 + context_claims;
    let mut boundary = Vec::with_capacity(boundary_claims);
    for weight in &row_weights {
        boundary.push(mul_ext(b, weight, &lane_at_s[2]));
        boundary.push(mul_ext(b, weight, &lane_at_s[3]));
    }
    boundary.push(mul_ext(b, &row_weights[0], &lane_at_s[0]));
    if bind_context_vector {
        for weight in &row_weights {
            boundary.push(mul_ext(b, weight, &lane_at_s[1]));
        }
    } else {
        boundary.push(mul_ext(b, &row_weights[0], &lane_at_s[1]));
    }
    let final_row = mul_ext(b, &s_final_region, &s_low_zero);
    boundary.push(mul_ext(b, &final_row, &lane_at_s[0]));
    assert_eq!(boundary.len(), boundary_claims);

    // Each new scalar claim and each matching relation functional extends one
    // existing eta-Horner fold. Count both sides exactly.
    let mut scalar_extension = ExtExpr::zero();
    let mut relation_extension = ExtExpr::zero();
    for value in &boundary {
        scalar_extension = value.add(&mul_ext(b, eta, &scalar_extension));
        relation_extension = value.add(&mul_ext(b, eta, &relation_extension));
    }

    let mut functionals = Vec::with_capacity(5 + boundary_claims);
    functionals.push(increment);
    functionals.extend(current_operands);
    functionals.extend(boundary);
    PostOutput {
        functionals,
        scalar_rlc_sink: scalar_extension,
        relation_rlc_sink: relation_extension,
    }
}

fn native_post_functionals(
    r: &[Block256; BANK_VARS],
    s: &[Block256; BANK_VARS],
    bind_context_vector: bool,
) -> Vec<Block256> {
    let eq_r = mle_weights_low_to_high(r);
    let eq_s = mle_weights_low_to_high(s);
    let mut operand_weights: [Vec<Block256>; 5] =
        std::array::from_fn(|_| vec![Block256::ZERO; 1 << BANK_VARS]);
    for row in POLICY_BASE_ROW..POLICY_BASE_ROW + PROGRAM_STEPS {
        for output_lane in 0..3 {
            if output_lane < 2 {
                operand_weights[0][4 * (row + 1) + output_lane] += eq_r[4 * row + output_lane];
            }
            for input_lane in 0..4 {
                operand_weights[1 + input_lane][4 * row + input_lane] +=
                    eq_r[4 * row + output_lane];
            }
        }
    }
    let mut result: Vec<Block256> = operand_weights
        .iter()
        .map(|weights| {
            weights
                .iter()
                .zip(&eq_s)
                .fold(Block256::ZERO, |sum, (weight, basis)| {
                    sum + *weight * *basis
                })
        })
        .collect();
    for row in POLICY_BASE_ROW..POLICY_BASE_ROW + PROGRAM_STEPS {
        result.push(eq_s[4 * row + 2]);
        result.push(eq_s[4 * row + 3]);
    }
    result.push(eq_s[4 * POLICY_BASE_ROW]);
    if bind_context_vector {
        for row in POLICY_BASE_ROW..POLICY_BASE_ROW + PROGRAM_STEPS {
            result.push(eq_s[4 * row + 1]);
        }
    } else {
        result.push(eq_s[4 * POLICY_BASE_ROW + 1]);
    }
    result.push(eq_s[4 * FINAL_ROW]);
    let context_claims = if bind_context_vector {
        PROGRAM_STEPS
    } else {
        1
    };
    assert_eq!(
        result.len(),
        5 + PROGRAM_BOUNDARY_CLAIMS + 2 + context_claims
    );
    result
}

fn run(bind_context_vector: bool) -> Value {
    let r_native: [Block256; BANK_VARS] =
        std::array::from_fn(|index| wide(index + 32, 0x554E_4956_5250_0001));
    let s_native: [Block256; BANK_VARS] =
        std::array::from_fn(|index| wide(index + 64, 0x554E_4956_5350_0001));
    let operands_native: [Block256; 5] =
        std::array::from_fn(|index| wide(index + 96, 0x554E_4956_4F50_0001));
    let eta_native = wide(127, 0x554E_4956_4554_0001);

    let mut b = FieldR1csBuilder::new();
    let r: [ExtExpr; BANK_VARS] =
        std::array::from_fn(|index| alloc_block256(&mut b, r_native[index]));
    let s: [ExtExpr; BANK_VARS] =
        std::array::from_fn(|index| alloc_block256(&mut b, s_native[index]));
    let operands: [ExtExpr; 5] =
        std::array::from_fn(|index| alloc_block256(&mut b, operands_native[index]));
    let eta = alloc_block256(&mut b, eta_native);
    constrain_nonzero_ext(&mut b, &eta);

    // These aliases are already present in the production terminal/post-claim
    // verifier. Build them before the measured delta.
    let row_eq_r = eq_ind_partial_eval_ext_trace(&mut b, &r[ROW_START..ROW_END]);
    let lane_at_r = lane_basis(&mut b, &r);
    let lane_at_s = lane_basis(&mut b, &s);
    let r_bank_high = bank_high_zero(&mut b, &r);
    let s_bank_high = bank_high_zero(&mut b, &s);

    let terminal_start = b.num_wires();
    let terminal =
        universal_contract_terminal(&mut b, &operands, &row_eq_r, &lane_at_r, &r_bank_high);
    let terminal_rows = b.num_wires() - terminal_start;
    assert_eq!(
        terminal.eval(b.values()),
        F256::from_tower(universal_contract_terminal_native(
            &r_native,
            &operands_native
        ))
    );

    let post_start = b.num_wires();
    let post = universal_contract_post_delta(
        &mut b,
        &r,
        &s,
        &lane_at_r,
        &lane_at_s,
        &r_bank_high,
        &s_bank_high,
        &eta,
        bind_context_vector,
    );
    let post_rows = b.num_wires() - post_start;
    let expected = native_post_functionals(&r_native, &s_native, bind_context_vector);
    assert_eq!(post.functionals.len(), expected.len());
    for (index, (actual, expected)) in post.functionals.iter().zip(expected).enumerate() {
        assert_eq!(
            actual.eval(b.values()),
            F256::from_tower(expected),
            "post functional {index}"
        );
    }
    let _sink = post
        .functionals
        .iter()
        .fold(terminal, |sum, value| sum.add(value))
        .add(&post.scalar_rlc_sink)
        .add(&post.relation_rlc_sink);
    let (r1cs, witness) = b.build();
    assert!(r1cs.satisfies(&witness));

    let per_tile = terminal_rows + post_rows;
    let b25_four = B25_USEFUL_ROWS + B25_CANDIDATE_CONTRACT_SLOTS * per_tile;
    let b25_all_body = B25_USEFUL_ROWS + B25_BODY_SLOTS * per_tile;
    let b255_32 = B255_USEFUL_ROWS + B255_CANDIDATE_CONTRACT_SLOTS * per_tile;
    let b255_all_body = B255_USEFUL_ROWS + B255_BODY_SLOTS * per_tile;
    let context_claims = if bind_context_vector {
        PROGRAM_STEPS
    } else {
        1
    };
    let boundary_claims = PROGRAM_BOUNDARY_CLAIMS + 2 + context_claims;
    json!({
        "context_mode": if bind_context_vector { "one body or block context value per program step" } else { "one context value shared by all program steps" },
        "program_steps": PROGRAM_STEPS,
        "instruction_encoding": "two GF(2^128) cells: opcode in {0,1,2,3}, unrestricted operand",
        "program_bytes_before_outer_hashing": PROGRAM_STEPS * 2 * 16,
        "same_five_terminal_operand_claims": true,
        "program_and_semantic_sparse_boundary_claims": boundary_claims,
        "terminal_rows": terminal_rows,
        "post_claim_rows": post_rows,
        "incremental_rows_per_contract_capable_authorization_tile": per_tile,
        "b25_current_useful_rows": B25_USEFUL_ROWS,
        "b25_four_contract_capable_tiles_projected_useful_rows_before_history_links": b25_four,
        "b25_four_contract_capable_tiles_rows_left_before_history_links": B25_PADDED_ROWS.saturating_sub(b25_four),
        "b25_all_25_body_tiles_projected_useful_rows": b25_all_body,
        "b25_all_25_body_tiles_cross_m22": b25_all_body > B25_PADDED_ROWS,
        "b255_current_useful_rows": B255_USEFUL_ROWS,
        "b255_32_contract_capable_tiles_projected_useful_rows_before_history_links": b255_32,
        "b255_32_contract_capable_tiles_rows_left_before_history_links": B255_PADDED_ROWS.saturating_sub(b255_32),
        "b255_all_255_body_tiles_projected_useful_rows": b255_all_body,
        "b255_all_255_body_tiles_rows_left": B255_PADDED_ROWS.saturating_sub(b255_all_body),
        "r1cs_satisfied": true,
        "native_terminal_and_all_post_functionals_match": true
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
        "usage: universal_contract_rows [NEW_JSON_FILE]"
    );
    let result = json!({
        "schema": 1,
        "unix_time": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "source_revision": String::from_utf8(
            std::process::Command::new("git").args(["rev-parse", "HEAD"]).output().unwrap().stdout
        ).unwrap().trim(),
        "kind": "universal_in_trace_program_recursive_row_model",
        "scalar_context_case": run(false),
        "context_vector_case": run(true),
        "established": [
            "opcode and operand are proof witness cells rather than verifier-selected program coefficients",
            "the fixed relation constrains opcodes to exactly four values",
            "two different programs use the same relation shape",
            "all sixteen program cells, old State, every selected context value, and new State have sparse boundary functionals",
            "eight independently bound context cells preserve the same five terminal dynamic operand claims",
            "the existing five terminal operand claims suffice",
            "the generated R1CS witness satisfies its matrix and matches native functionals"
        ],
        "not_established": [
            "complete native PCS proof for this universal relation",
            "mixed wallet/contract region-sidecar layout",
            "integrated HistoryStep links and exact row count",
            "outer BlockSpine program/object descriptor and root pins",
            "contract ABI, integer semantics, storage, calls, concurrency, or soundness composition",
            "production timings"
        ]
    });
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    if let Some(path) = args.get(1) {
        save_new(path, &result);
    }
}
