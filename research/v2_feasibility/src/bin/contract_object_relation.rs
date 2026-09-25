//! Research-only native relation for a compact proof-native contract object.
//!
//! The object owner is a 256-bit Poseidon2b commitment to an exact 16-byte
//! program, one 128-bit state word, and a 256-bit controller address. The
//! relation proves the old and new commitments plus eight bounded execution
//! steps in the candidate 1267-cell capsule layout. It does not construct a
//! PCS proof or integrate the relation into HistoryStep.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use noid_core::{Block128, Block256, TowerField};
use noid_gkr::zk_auth_capsule::{evaluate_mle_low_to_high, mle_weights_low_to_high};
use noid_gkr::{evaluate_permutation, PermLayerWitness};
use noid_poseidon2b::native::domain::{capacity_iv, DomainTag};
use noid_poseidon2b::native::permutation::{
    F_ROUNDS, MDS_FULL, MDS_PARTIAL, N_ROUNDS, P_ROUNDS, ROUND_CONSTANTS,
};
use serde_json::{json, Value};

const BANK_LEN: usize = 2_048;
const BANK_VARS: usize = 11;
const LANES: usize = 4;
const STORED_PERMUTATION_ROWS: usize = N_ROUNDS + 1;
const PROGRAM_STEPS: usize = 8;
const VM_ROWS: usize = PROGRAM_STEPS + 1;
const SOURCE_COIN_START: usize = 1_024;
const SOURCE_COIN_LEN: usize = 520;
const LIBRA_START: usize = 512;
const LIBRA_LEN: usize = 256;
const TERMINAL_PAD_START: usize = 768;
const TERMINAL_PAD_LEN: usize = 5;
const DUMMY_RELATION_INDEX: usize = BANK_LEN - 1;
const CONTRACT_DOMAIN: DomainTag = DomainTag::new(b"CNTRCT__");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Opcode {
    Nop = 0,
    AddImmediate = 1,
    MultiplyImmediate = 2,
    SelectContextImmediate = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Instruction {
    opcode: Opcode,
    immediate: u16,
}

impl Instruction {
    fn encode(self) -> u16 {
        ((self.immediate & 0x3fff) << 2) | self.opcode as u16
    }

    fn decode(encoded: u16) -> Self {
        let opcode = match encoded & 3 {
            0 => Opcode::Nop,
            1 => Opcode::AddImmediate,
            2 => Opcode::MultiplyImmediate,
            3 => Opcode::SelectContextImmediate,
            _ => unreachable!(),
        };
        Self {
            opcode,
            immediate: encoded >> 2,
        }
    }
}

fn encode_program(program: &[Instruction; PROGRAM_STEPS]) -> Block128 {
    let mut bytes = [0u8; 16];
    for (index, instruction) in program.iter().enumerate() {
        bytes[2 * index..2 * index + 2].copy_from_slice(&instruction.encode().to_le_bytes());
    }
    Block128::from(u128::from_le_bytes(bytes))
}

fn decode_program(word: Block128) -> [Instruction; PROGRAM_STEPS] {
    let bytes = word.to_u128().to_le_bytes();
    std::array::from_fn(|index| {
        Instruction::decode(u16::from_le_bytes([bytes[2 * index], bytes[2 * index + 1]]))
    })
}

fn execute_step(accumulator: Block128, context: Block128, instruction: Instruction) -> Block128 {
    let immediate = Block128::from(instruction.immediate as u128);
    match instruction.opcode {
        Opcode::Nop => accumulator,
        Opcode::AddImmediate => accumulator + immediate,
        Opcode::MultiplyImmediate => accumulator * immediate,
        Opcode::SelectContextImmediate => accumulator + context * (accumulator + immediate),
    }
}

fn execute_program(
    mut accumulator: Block128,
    context: Block128,
    program: &[Instruction; PROGRAM_STEPS],
) -> Block128 {
    for &instruction in program {
        accumulator = execute_step(accumulator, context, instruction);
    }
    accumulator
}

fn apply_mds(input: [Block128; LANES]) -> [Block128; LANES] {
    std::array::from_fn(|output_lane| {
        (0..LANES).fold(Block128::ZERO, |sum, input_lane| {
            sum + Block128::from(MDS_FULL[output_lane][input_lane]) * input[input_lane]
        })
    })
}

fn object_permutations(
    program: Block128,
    state: Block128,
    controller: [Block128; 2],
) -> (PermLayerWitness, PermLayerWitness) {
    let iv = capacity_iv(CONTRACT_DOMAIN);
    let first = evaluate_permutation([program, state, iv[0], iv[1]]);
    let first_output = first.final_state();
    let second = evaluate_permutation([
        first_output[0] + controller[0],
        first_output[1] + controller[1],
        first_output[2],
        first_output[3],
    ]);
    (first, second)
}

fn object_root(program: Block128, state: Block128, controller: [Block128; 2]) -> [Block128; 2] {
    let (_, second) = object_permutations(program, state, controller);
    let output = second.final_state();
    [output[0], output[1]]
}

#[derive(Clone, Debug)]
struct Layout {
    permutation_rows: [[usize; STORED_PERMUTATION_ROWS]; 4],
    vm_rows: [usize; VM_ROWS],
    all_trace_rows: Vec<usize>,
}

impl Layout {
    fn selected() -> Self {
        // Cells 512..767 remain the Libra mask, 768..772 the five terminal
        // pads, and 1024..1543 the shortened source-opening coin block. All
        // selected rows are four-cell aligned.
        let mut rows = Vec::new();
        rows.extend(0..128);
        rows.extend(194..256);
        rows.extend(386..512);
        assert_eq!(rows.len(), 316);

        let mut cursor = 0;
        let permutation_rows = std::array::from_fn(|_| {
            std::array::from_fn(|_| {
                let row = rows[cursor];
                cursor += 1;
                row
            })
        });
        let vm_rows = std::array::from_fn(|_| {
            let row = rows[cursor];
            cursor += 1;
            row
        });
        let all_trace_rows = rows[..cursor].to_vec();
        assert_eq!(cursor, 4 * STORED_PERMUTATION_ROWS + VM_ROWS);
        Self {
            permutation_rows,
            vm_rows,
            all_trace_rows,
        }
    }

    fn trace_cells(&self) -> usize {
        self.all_trace_rows.len() * LANES
    }

    fn jump_edges(&self) -> Vec<[usize; 2]> {
        self.permutation_rows
            .iter()
            .flat_map(|rows| rows.windows(2))
            .filter(|edge| edge[1] != edge[0] + 1)
            .map(|edge| [edge[0], edge[1]])
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Statement {
    program: Block128,
    current: Block128,
    next: Block128,
    context: Block128,
    call_nonce: Block128,
    controller: [Block128; 2],
    old_root: [Block128; 2],
    new_root: [Block128; 2],
}

#[derive(Clone)]
struct Case {
    name: &'static str,
    program: [Instruction; PROGRAM_STEPS],
    current: Block128,
    context: Block128,
    call_nonce: Block128,
    controller: [Block128; 2],
}

impl Case {
    fn statement(&self) -> Statement {
        let program = encode_program(&self.program);
        let next = execute_program(self.current, self.context, &self.program);
        Statement {
            program,
            current: self.current,
            next,
            context: self.context,
            call_nonce: self.call_nonce,
            controller: self.controller,
            old_root: object_root(program, self.current, self.controller),
            new_root: object_root(program, next, self.controller),
        }
    }
}

fn cell(row: usize, lane: usize) -> usize {
    assert!(row < BANK_LEN / LANES && lane < LANES);
    LANES * row + lane
}

fn deterministic_cell(index: usize, domain: u128) -> Block128 {
    let value = domain
        .wrapping_mul(index as u128 + 1)
        .rotate_left(((17 * index + 11) % 127) as u32)
        ^ (index as u128 + 19).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let value = Block128::from(value);
    if value == Block128::ZERO || value == Block128::ONE {
        value + Block128::from(2u128)
    } else {
        value
    }
}

fn wide(index: usize, domain: u128) -> Block256 {
    Block256::from_raw_challenge_lanes(
        deterministic_cell(2 * index, domain),
        deterministic_cell(2 * index + 1, domain ^ 0xC1_0256),
    )
}

fn write_permutation(
    bank: &mut [Block128],
    rows: &[usize; STORED_PERMUTATION_ROWS],
    witness: &PermLayerWitness,
) {
    assert_eq!(witness.state.len(), STORED_PERMUTATION_ROWS);
    for (logical_row, &physical_row) in rows.iter().enumerate() {
        for lane in 0..LANES {
            bank[cell(physical_row, lane)] = witness.state[logical_row][lane];
        }
    }
}

fn build_bank(case: &Case, layout: &Layout) -> (Vec<Block128>, Statement) {
    let statement = case.statement();
    let mut bank: Vec<_> = (0..BANK_LEN)
        .map(|index| deterministic_cell(index, 0xC017_7AC7_0000_0001))
        .collect();
    let (old_first, old_second) =
        object_permutations(statement.program, statement.current, statement.controller);
    let (new_first, new_second) =
        object_permutations(statement.program, statement.next, statement.controller);
    for (rows, witness) in
        layout
            .permutation_rows
            .iter()
            .zip([&old_first, &old_second, &new_first, &new_second])
    {
        write_permutation(&mut bank, rows, witness);
    }

    let mut vm_state = [
        statement.current,
        statement.context,
        statement.program,
        statement.call_nonce,
    ];
    for lane in 0..LANES {
        bank[cell(layout.vm_rows[0], lane)] = vm_state[lane];
    }
    for (step, &instruction) in case.program.iter().enumerate() {
        vm_state[0] = execute_step(vm_state[0], vm_state[1], instruction);
        for lane in 0..LANES {
            bank[cell(layout.vm_rows[step + 1], lane)] = vm_state[lane];
        }
    }
    assert_eq!(vm_state[0], statement.next);
    (bank, statement)
}

fn is_partial_round(round: usize) -> bool {
    (F_ROUNDS / 2..F_ROUNDS / 2 + P_ROUNDS).contains(&round)
}

fn sigma_at(round: usize, lane: usize) -> Block128 {
    if is_partial_round(round) && lane != 0 {
        Block128::ZERO
    } else {
        Block128::ONE
    }
}

fn round_constant_at(round: usize, lane: usize) -> Block128 {
    if is_partial_round(round) && lane != 0 {
        Block128::ZERO
    } else {
        Block128::from(ROUND_CONSTANTS[lane][round])
    }
}

fn mds_at(round: usize, output_lane: usize, input_lane: usize) -> Block128 {
    if is_partial_round(round) {
        Block128::from(MDS_PARTIAL[output_lane][input_lane])
    } else {
        Block128::from(MDS_FULL[output_lane][input_lane])
    }
}

fn pow7_base(value: Block128) -> Block128 {
    let square = value * value;
    let fourth = square * square;
    fourth * square * value
}

fn pow7_wide(value: Block256) -> Block256 {
    let square = value * value;
    let fourth = square * square;
    fourth * square * value
}

#[derive(Clone)]
struct PublicTables {
    active: Vec<Block128>,
    poseidon_mds: [Vec<Block128>; LANES],
    poseidon_sigma: [Vec<Block128>; LANES],
    poseidon_rc: [Vec<Block128>; LANES],
    vm_coefficient: [Vec<Block128>; LANES],
    vm_cross_01: Vec<Block128>,
    vm_constant: Vec<Block128>,
}

impl PublicTables {
    fn build(layout: &Layout, program_word: Block128) -> Self {
        let mut active = vec![Block128::ZERO; BANK_LEN];
        let mut poseidon_mds = std::array::from_fn(|_| vec![Block128::ZERO; BANK_LEN]);
        let mut poseidon_sigma = std::array::from_fn(|_| vec![Block128::ZERO; BANK_LEN]);
        let mut poseidon_rc = std::array::from_fn(|_| vec![Block128::ZERO; BANK_LEN]);
        let mut vm_coefficient = std::array::from_fn(|_| vec![Block128::ZERO; BANK_LEN]);
        let mut vm_cross_01 = vec![Block128::ZERO; BANK_LEN];
        let mut vm_constant = vec![Block128::ZERO; BANK_LEN];

        for rows in &layout.permutation_rows {
            for (round, &physical_row) in rows.iter().take(N_ROUNDS).enumerate() {
                for output_lane in 0..LANES {
                    let index = cell(physical_row, output_lane);
                    assert_eq!(active[index], Block128::ZERO);
                    active[index] = Block128::ONE;
                    for input_lane in 0..LANES {
                        poseidon_mds[input_lane][index] = mds_at(round, output_lane, input_lane);
                        poseidon_sigma[input_lane][index] = sigma_at(round, input_lane);
                        poseidon_rc[input_lane][index] = round_constant_at(round, input_lane);
                    }
                }
            }
        }

        let program = decode_program(program_word);
        for (step, &instruction) in program.iter().enumerate() {
            let physical_row = layout.vm_rows[step];
            for output_lane in 0..LANES {
                let index = cell(physical_row, output_lane);
                assert_eq!(active[index], Block128::ZERO);
                active[index] = Block128::ONE;
                if output_lane == 0 {
                    let immediate = Block128::from(instruction.immediate as u128);
                    match instruction.opcode {
                        Opcode::Nop => vm_coefficient[0][index] = Block128::ONE,
                        Opcode::AddImmediate => {
                            vm_coefficient[0][index] = Block128::ONE;
                            vm_constant[index] = immediate;
                        }
                        Opcode::MultiplyImmediate => vm_coefficient[0][index] = immediate,
                        Opcode::SelectContextImmediate => {
                            vm_coefficient[0][index] = Block128::ONE;
                            vm_coefficient[1][index] = immediate;
                            vm_cross_01[index] = Block128::ONE;
                        }
                    }
                } else {
                    vm_coefficient[output_lane][index] = Block128::ONE;
                }
            }
        }
        Self {
            active,
            poseidon_mds,
            poseidon_sigma,
            poseidon_rc,
            vm_coefficient,
            vm_cross_01,
            vm_constant,
        }
    }
}

#[derive(Clone)]
struct RelationTables {
    public: PublicTables,
    increment: Vec<Block128>,
    lane: [Vec<Block128>; LANES],
}

impl RelationTables {
    fn build(bank: &[Block128], layout: &Layout, program_word: Block128) -> Self {
        let public = PublicTables::build(layout, program_word);
        let mut increment = vec![Block128::ZERO; BANK_LEN];
        let mut lane = std::array::from_fn(|_| vec![Block128::ZERO; BANK_LEN]);

        for rows in &layout.permutation_rows {
            for round in 0..N_ROUNDS {
                for output_lane in 0..LANES {
                    let index = cell(rows[round], output_lane);
                    increment[index] = bank[cell(rows[round + 1], output_lane)];
                    for input_lane in 0..LANES {
                        lane[input_lane][index] = bank[cell(rows[round], input_lane)];
                    }
                }
            }
        }
        for step in 0..PROGRAM_STEPS {
            for output_lane in 0..LANES {
                let index = cell(layout.vm_rows[step], output_lane);
                increment[index] = bank[cell(layout.vm_rows[step + 1], output_lane)];
                for input_lane in 0..LANES {
                    lane[input_lane][index] = bank[cell(layout.vm_rows[step], input_lane)];
                }
            }
        }
        increment[DUMMY_RELATION_INDEX] = bank[TERMINAL_PAD_START];
        for input_lane in 0..LANES {
            lane[input_lane][DUMMY_RELATION_INDEX] = bank[TERMINAL_PAD_START + 1 + input_lane];
        }
        Self {
            public,
            increment,
            lane,
        }
    }

    fn relation_at(&self, index: usize) -> Block128 {
        let mut expression = self.increment[index] + self.public.vm_constant[index];
        for input_lane in 0..LANES {
            let state = self.lane[input_lane][index];
            let sigma = self.public.poseidon_sigma[input_lane][index];
            let with_constant = state + self.public.poseidon_rc[input_lane][index];
            let poseidon = sigma * pow7_base(with_constant) + (Block128::ONE + sigma) * state;
            expression += self.public.poseidon_mds[input_lane][index] * poseidon;
            expression += self.public.vm_coefficient[input_lane][index] * state;
        }
        expression += self.public.vm_cross_01[index] * self.lane[0][index] * self.lane[1][index];
        self.public.active[index] * expression
    }

    fn validate(&self) -> bool {
        (0..BANK_LEN).all(|index| self.relation_at(index) == Block128::ZERO)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TerminalOperands {
    increment: Block256,
    lane: [Block256; LANES],
}

impl TerminalOperands {
    fn ordered(self) -> [Block256; 5] {
        [
            self.increment,
            self.lane[0],
            self.lane[1],
            self.lane[2],
            self.lane[3],
        ]
    }
}

fn terminal_operands(tables: &RelationTables, point: &[Block256; BANK_VARS]) -> TerminalOperands {
    TerminalOperands {
        increment: evaluate_mle_low_to_high(&tables.increment, point).unwrap(),
        lane: std::array::from_fn(|lane| {
            evaluate_mle_low_to_high(&tables.lane[lane], point).unwrap()
        }),
    }
}

fn public_terminal(
    public: &PublicTables,
    point: &[Block256; BANK_VARS],
    operands: TerminalOperands,
) -> Block256 {
    let active = evaluate_mle_low_to_high(&public.active, point).unwrap();
    let mut expression =
        operands.increment + evaluate_mle_low_to_high(&public.vm_constant, point).unwrap();
    for input_lane in 0..LANES {
        let state = operands.lane[input_lane];
        let sigma = evaluate_mle_low_to_high(&public.poseidon_sigma[input_lane], point).unwrap();
        let rc = evaluate_mle_low_to_high(&public.poseidon_rc[input_lane], point).unwrap();
        let mds = evaluate_mle_low_to_high(&public.poseidon_mds[input_lane], point).unwrap();
        expression += mds * (sigma * pow7_wide(state + rc) + (Block256::ONE + sigma) * state);
        let coefficient =
            evaluate_mle_low_to_high(&public.vm_coefficient[input_lane], point).unwrap();
        expression += coefficient * state;
    }
    let cross = evaluate_mle_low_to_high(&public.vm_cross_01, point).unwrap();
    expression += cross * operands.lane[0] * operands.lane[1];
    active * expression
}

#[derive(Clone, Debug)]
struct LinearClaim {
    terms: Vec<(usize, Block128)>,
    expected: Block128,
}

impl LinearClaim {
    fn evaluate(&self, bank: &[Block128]) -> Block128 {
        self.terms
            .iter()
            .fold(Block128::ZERO, |sum, &(index, coefficient)| {
                sum + coefficient * bank[index]
            })
    }
}

fn singleton_claim(index: usize, expected: Block128) -> LinearClaim {
    LinearClaim {
        terms: vec![(index, Block128::ONE)],
        expected,
    }
}

fn append_object_claims(
    claims: &mut Vec<LinearClaim>,
    rows: &[[usize; STORED_PERMUTATION_ROWS]],
    program: Block128,
    state: Block128,
    controller: [Block128; 2],
    root: [Block128; 2],
) {
    let iv = capacity_iv(CONTRACT_DOMAIN);
    let expected_first = apply_mds([program, state, iv[0], iv[1]]);
    for lane in 0..LANES {
        claims.push(singleton_claim(
            cell(rows[0][0], lane),
            expected_first[lane],
        ));
    }

    // second.row0 = MDS(first.output + [controller_hi, controller_lo, 0, 0]).
    for output_lane in 0..LANES {
        let mut terms = vec![(cell(rows[1][0], output_lane), Block128::ONE)];
        for input_lane in 0..LANES {
            terms.push((
                cell(rows[0][N_ROUNDS], input_lane),
                Block128::from(MDS_FULL[output_lane][input_lane]),
            ));
        }
        let expected = Block128::from(MDS_FULL[output_lane][0]) * controller[0]
            + Block128::from(MDS_FULL[output_lane][1]) * controller[1];
        claims.push(LinearClaim { terms, expected });
    }
    for lane in 0..2 {
        claims.push(singleton_claim(cell(rows[1][N_ROUNDS], lane), root[lane]));
    }
}

fn boundary_claims(layout: &Layout, statement: Statement) -> Vec<LinearClaim> {
    let mut claims = Vec::new();
    append_object_claims(
        &mut claims,
        &layout.permutation_rows[..2],
        statement.program,
        statement.current,
        statement.controller,
        statement.old_root,
    );
    append_object_claims(
        &mut claims,
        &layout.permutation_rows[2..],
        statement.program,
        statement.next,
        statement.controller,
        statement.new_root,
    );
    let initial = [
        statement.current,
        statement.context,
        statement.program,
        statement.call_nonce,
    ];
    let final_state = [
        statement.next,
        statement.context,
        statement.program,
        statement.call_nonce,
    ];
    for lane in 0..LANES {
        claims.push(singleton_claim(
            cell(layout.vm_rows[0], lane),
            initial[lane],
        ));
        claims.push(singleton_claim(
            cell(layout.vm_rows[PROGRAM_STEPS], lane),
            final_state[lane],
        ));
    }
    assert_eq!(claims.len(), 28);
    claims
}

fn operand_functional_weights(
    layout: &Layout,
    point: &[Block256; BANK_VARS],
) -> [Vec<Block256>; 5] {
    let eq = mle_weights_low_to_high(point);
    let mut weights = std::array::from_fn(|_| vec![Block256::ZERO; BANK_LEN]);
    for rows in &layout.permutation_rows {
        for round in 0..N_ROUNDS {
            for output_lane in 0..LANES {
                let relation_index = cell(rows[round], output_lane);
                weights[0][cell(rows[round + 1], output_lane)] += eq[relation_index];
                for input_lane in 0..LANES {
                    weights[1 + input_lane][cell(rows[round], input_lane)] += eq[relation_index];
                }
            }
        }
    }
    for step in 0..PROGRAM_STEPS {
        for output_lane in 0..LANES {
            let relation_index = cell(layout.vm_rows[step], output_lane);
            weights[0][cell(layout.vm_rows[step + 1], output_lane)] += eq[relation_index];
            for input_lane in 0..LANES {
                weights[1 + input_lane][cell(layout.vm_rows[step], input_lane)] +=
                    eq[relation_index];
            }
        }
    }
    weights[0][TERMINAL_PAD_START] += eq[DUMMY_RELATION_INDEX];
    for input_lane in 0..LANES {
        weights[1 + input_lane][TERMINAL_PAD_START + 1 + input_lane] += eq[DUMMY_RELATION_INDEX];
    }
    weights
}

fn verify_post_claim(
    bank: &[Block128],
    layout: &Layout,
    point: &[Block256; BANK_VARS],
    operands: TerminalOperands,
    claims: &[LinearClaim],
    eta: Block256,
) -> bool {
    let operand_weights = operand_functional_weights(layout, point);
    let mut combined_weights = vec![Block256::ZERO; BANK_LEN];
    let mut combined_expected = Block256::ZERO;
    let mut power = Block256::ONE;
    let mut accumulate = |weights: &[(usize, Block256)], expected: Block256| {
        for &(index, coefficient) in weights {
            combined_weights[index] += power * coefficient;
        }
        combined_expected += power * expected;
        power *= eta;
    };
    for (weights, value) in operand_weights.iter().zip(operands.ordered()) {
        let sparse: Vec<_> = weights
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, coefficient)| *coefficient != Block256::ZERO)
            .collect();
        accumulate(&sparse, value);
    }
    for claim in claims {
        let sparse: Vec<_> = claim
            .terms
            .iter()
            .map(|&(index, coefficient)| (index, Block256::from(coefficient)))
            .collect();
        accumulate(&sparse, Block256::from(claim.expected));
    }
    let actual = bank
        .iter()
        .copied()
        .zip(&combined_weights)
        .fold(Block256::ZERO, |sum, (value, &weight)| {
            sum + Block256::from(value) * weight
        });
    actual == combined_expected
}

fn verify_candidate(
    bank: &[Block128],
    layout: &Layout,
    statement: Statement,
    point: &[Block256; BANK_VARS],
    eta: Block256,
) -> bool {
    if statement.context != Block128::ZERO && statement.context != Block128::ONE {
        return false;
    }
    let tables = RelationTables::build(bank, layout, statement.program);
    if !tables.validate() {
        return false;
    }
    let claims = boundary_claims(layout, statement);
    if claims
        .iter()
        .any(|claim| claim.evaluate(bank) != claim.expected)
    {
        return false;
    }
    let operands = terminal_operands(&tables, point);
    // This is the terminal value of the non-multilinear composition after
    // sumcheck reduction. It is generally nonzero away from the Boolean cube;
    // a complete capsule carries the round polynomials from initial claim zero
    // to this value.
    let _terminal_value = public_terminal(&tables.public, point, operands);
    verify_post_claim(bank, layout, point, operands, &claims, eta)
}

fn mutate(value: Block128, salt: u128) -> Block128 {
    value + Block128::from(salt)
}

fn run_case(case: &Case, layout: &Layout) -> Value {
    let started = Instant::now();
    let (bank, statement) = build_bank(case, layout);
    let build_ms = started.elapsed().as_millis();
    let point = std::array::from_fn(|index| wide(index, 0xC017_7AC7_1000_0001));
    let eta = wide(31, 0xC017_7AC7_2000_0001);
    assert_ne!(eta, Block256::ZERO);

    let verify_started = Instant::now();
    let accepts_valid = verify_candidate(&bank, layout, statement, &point, eta);
    let verify_ms = verify_started.elapsed().as_millis();
    assert!(accepts_valid);

    let mut wrong_program = statement;
    wrong_program.program = mutate(wrong_program.program, 1);
    let mut wrong_current = statement;
    wrong_current.current = mutate(wrong_current.current, 2);
    let mut wrong_next = statement;
    wrong_next.next = mutate(wrong_next.next, 4);
    let mut wrong_context = statement;
    wrong_context.context = mutate(wrong_context.context, 2);
    let mut wrong_nonce = statement;
    wrong_nonce.call_nonce = mutate(wrong_nonce.call_nonce, 8);
    let mut wrong_controller = statement;
    wrong_controller.controller[0] = mutate(wrong_controller.controller[0], 16);
    let mut wrong_old_root = statement;
    wrong_old_root.old_root[1] = mutate(wrong_old_root.old_root[1], 32);
    let mut wrong_new_root = statement;
    wrong_new_root.new_root[0] = mutate(wrong_new_root.new_root[0], 64);
    let mut corrupted_bank = bank.clone();
    corrupted_bank[cell(layout.permutation_rows[2][17], 1)] =
        mutate(corrupted_bank[cell(layout.permutation_rows[2][17], 1)], 128);

    let rejection_cases = [
        (
            "wrong-program",
            !verify_candidate(&bank, layout, wrong_program, &point, eta),
        ),
        (
            "wrong-current",
            !verify_candidate(&bank, layout, wrong_current, &point, eta),
        ),
        (
            "wrong-next",
            !verify_candidate(&bank, layout, wrong_next, &point, eta),
        ),
        (
            "wrong-context",
            !verify_candidate(&bank, layout, wrong_context, &point, eta),
        ),
        (
            "wrong-nonce",
            !verify_candidate(&bank, layout, wrong_nonce, &point, eta),
        ),
        (
            "wrong-controller",
            !verify_candidate(&bank, layout, wrong_controller, &point, eta),
        ),
        (
            "wrong-old-root",
            !verify_candidate(&bank, layout, wrong_old_root, &point, eta),
        ),
        (
            "wrong-new-root",
            !verify_candidate(&bank, layout, wrong_new_root, &point, eta),
        ),
        (
            "corrupted-poseidon-trace",
            !verify_candidate(&corrupted_bank, layout, statement, &point, eta),
        ),
    ];
    assert!(rejection_cases.iter().all(|(_, rejected)| *rejected));

    let program_bytes = statement.program.to_u128().to_le_bytes();
    json!({
        "name": case.name,
        "program_hex": program_bytes.iter().rev().map(|byte| format!("{byte:02x}")).collect::<String>(),
        "current": format!("{:032x}", statement.current.to_u128()),
        "next": format!("{:032x}", statement.next.to_u128()),
        "context": format!("{:032x}", statement.context.to_u128()),
        "old_root": statement.old_root.map(|value| format!("{:032x}", value.to_u128())),
        "new_root": statement.new_root.map(|value| format!("{:032x}", value.to_u128())),
        "build_ms": build_ms,
        "native_relation_verify_ms": verify_ms,
        "valid_relation_accepted": accepts_valid,
        "rejections": rejection_cases.iter().map(|(name, rejected)| json!({"name": name, "rejected": rejected})).collect::<Vec<_>>(),
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
    assert_eq!(N_ROUNDS, 66);
    let layout = Layout::selected();
    let cases = [
        Case {
            name: "arithmetic",
            program: [
                Instruction {
                    opcode: Opcode::AddImmediate,
                    immediate: 7,
                },
                Instruction {
                    opcode: Opcode::MultiplyImmediate,
                    immediate: 11,
                },
                Instruction {
                    opcode: Opcode::AddImmediate,
                    immediate: 19,
                },
                Instruction {
                    opcode: Opcode::MultiplyImmediate,
                    immediate: 3,
                },
                Instruction {
                    opcode: Opcode::Nop,
                    immediate: 0,
                },
                Instruction {
                    opcode: Opcode::AddImmediate,
                    immediate: 29,
                },
                Instruction {
                    opcode: Opcode::MultiplyImmediate,
                    immediate: 5,
                },
                Instruction {
                    opcode: Opcode::Nop,
                    immediate: 0,
                },
            ],
            current: deterministic_cell(1, 0xA11C_E001),
            context: Block128::ZERO,
            call_nonce: deterministic_cell(2, 0xA11C_E001),
            controller: [
                deterministic_cell(3, 0xA11C_E001),
                deterministic_cell(4, 0xA11C_E001),
            ],
        },
        Case {
            name: "context-select",
            program: [
                Instruction {
                    opcode: Opcode::SelectContextImmediate,
                    immediate: 42,
                },
                Instruction {
                    opcode: Opcode::AddImmediate,
                    immediate: 5,
                },
                Instruction {
                    opcode: Opcode::MultiplyImmediate,
                    immediate: 9,
                },
                Instruction {
                    opcode: Opcode::SelectContextImmediate,
                    immediate: 77,
                },
                Instruction {
                    opcode: Opcode::AddImmediate,
                    immediate: 13,
                },
                Instruction {
                    opcode: Opcode::Nop,
                    immediate: 0,
                },
                Instruction {
                    opcode: Opcode::MultiplyImmediate,
                    immediate: 17,
                },
                Instruction {
                    opcode: Opcode::Nop,
                    immediate: 0,
                },
            ],
            current: deterministic_cell(5, 0xC07E_8701),
            context: Block128::ONE,
            call_nonce: deterministic_cell(6, 0xC07E_8701),
            controller: [
                deterministic_cell(7, 0xC07E_8701),
                deterministic_cell(8, 0xC07E_8701),
            ],
        },
    ];
    let results = cases
        .iter()
        .map(|case| run_case(case, &layout))
        .collect::<Vec<_>>();
    assert_ne!(cases[0].statement().program, cases[1].statement().program);
    assert_ne!(cases[0].statement().old_root, cases[1].statement().old_root);

    let trace_cells = layout.trace_cells();
    let candidate_trace_capacity = 512
        + (SOURCE_COIN_START - TERMINAL_PAD_START - TERMINAL_PAD_LEN)
        + (BANK_LEN - SOURCE_COIN_START - SOURCE_COIN_LEN);
    assert_eq!(candidate_trace_capacity, 1_267);
    assert_eq!(trace_cells, 1_108);
    let result = json!({
        "kind": "v2-contract-object-native-relation",
        "status": "native-relation-established-pcs-and-recursion-not-established",
        "source_revision": source_revision(),
        "generated_unix_seconds": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "object_model": {
            "owner_commitment_fields": ["exact-program-word-128", "state-word-128", "controller-hi-128", "controller-lo-128"],
            "owner_commitment_bits": 256,
            "program_encoding_bytes": 16,
            "program_steps": PROGRAM_STEPS,
            "program_whitelist": false,
            "persistent_state_bits": 128,
            "controller_bits": 256,
            "poseidon_permutations_old_and_new": 4,
        },
        "bank_layout": {
            "bank_cells": BANK_LEN,
            "libra_reserved_range": [LIBRA_START, LIBRA_START + LIBRA_LEN],
            "terminal_pad_reserved_range": [TERMINAL_PAD_START, TERMINAL_PAD_START + TERMINAL_PAD_LEN],
            "source_coin_reserved_range": [SOURCE_COIN_START, SOURCE_COIN_START + SOURCE_COIN_LEN],
            "candidate_trace_capacity_cells": candidate_trace_capacity,
            "used_trace_cells": trace_cells,
            "remaining_trace_cells": candidate_trace_capacity - trace_cells,
            "active_relation_equations": 4 * N_ROUNDS * LANES + PROGRAM_STEPS * LANES,
            "boundary_linear_claims": 28,
            "fragment_jump_edges": layout.jump_edges(),
        },
        "cases": results,
        "established": [
            "Four complete production-parameter Poseidon2b traces and an eight-step dynamic program fit in the candidate bank with 159 trace cells left.",
            "The exact 16-byte program is committed directly; the two tested programs use one verifier relation and no consensus whitelist.",
            "The relation binds old and new object roots, current and next state, controller, context, and call nonce.",
            "The five terminal operands suffice for all four Poseidon traces and the program trace.",
            "All Boolean equations, random-point terminal evaluation, and one RLC post-claim against the fragmented committed bank agree.",
            "Every tested statement or trace mutation is rejected.",
        ],
        "not_established": [
            "A zero-knowledge PCS proof using the shortened 520-coin layout.",
            "Recursive verifier row count for the fragmented post-claim map and 28 boundaries.",
            "Binding the public statement to exact Tx8x2 fields inside HistoryStep.",
            "Asset conservation, output policy, height derivation, or a production contract ABI.",
            "General memory, calls, unbounded execution, or programs longer than 16 bytes.",
            "A complete soundness or hiding certificate for this contract mode.",
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
