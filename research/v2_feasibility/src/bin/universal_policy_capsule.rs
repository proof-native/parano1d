//! Research-only composite owner + universal programmable-policy authorization capsule.
//!
//! This is not a transaction format or a consensus implementation. It asks
//! one narrow question: can the current private owner permutation and two
//! different bounded programs, committed inside the proof rather than decoded
//! into verifier-selected tables, share the existing 2^11-cell authorization
//! bank, one degree-ten MLE-check, five terminal operand claims, and the
//! complete existing PCS geometry?

use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use noid_core::mle::evaluate::evaluate_slice;
use noid_core::sumcheck::RoundPolynomial;
use noid_core::{Block128, Block256, TowerField};
use noid_fri_binius::zk_capsule_algebra::{
    evaluate_upper_at_low8, tail16_local_fold, FINAL_H_SYMBOLS, MID_STANDARD_FOLDS,
    PHASE_B_HIGH_VARS, PHASE_B_LOW_VARS, SOURCE_STANDARD_FOLDS, TAIL_SYMBOLS,
};
use noid_fri_binius::zk_capsule_pcs::{
    zk_capsule_pcs_bind_owner, zk_capsule_pcs_bind_phase_a, zk_capsule_pcs_commit_fresh,
    zk_capsule_pcs_commit_mid, zk_capsule_pcs_link_phase_b, zk_capsule_pcs_open,
    zk_capsule_pcs_prove_phase_a, zk_capsule_pcs_reveal_tail, zk_capsule_pcs_verify,
    ZkCapsulePcsMidCommitment, ZkCapsulePcsTailReveal,
};
use noid_fri_binius::zk_phase_a::{verify_phase_a, ZkPhaseARelationClaims, PHASE_A_VARS};
use noid_gkr::evaluate_permutation;
use noid_gkr::zk_auth_capsule::{
    certify_terminal_blinding_rank, evaluate_mle_low_to_high, libra_mask_final_functional_weights,
    libra_mask_mle_functional_weights, mle_weights_low_to_high, sparse_boundary_claims,
    validate_sparse_boundary, AuthCapsuleBoundaryPublic, AuthCapsulePostClaimRelation,
    ZkAuthCapsuleBankView, ZK_AUTH_CAPSULE_ACTIVE_ROUNDS, ZK_AUTH_CAPSULE_BANK_LEN,
    ZK_AUTH_CAPSULE_BANK_VARS, ZK_AUTH_CAPSULE_LIBRA_MASK_LEN, ZK_AUTH_CAPSULE_LIBRA_MASK_OFFSET,
    ZK_AUTH_CAPSULE_PCS_COINS_OFFSET, ZK_AUTH_CAPSULE_REMAINING_PADDING_OFFSET,
    ZK_AUTH_CAPSULE_STATE_LEN, ZK_AUTH_CAPSULE_TERMINAL_BLINDING_OFFSET,
};
use noid_gkr::zk_auth_hiding::{
    certify_zk_auth_conditioned_companion_hyperplane, certify_zk_auth_joint_hiding_rank,
};
use noid_gkr::zk_authorization::{
    affine_blend_gamma_is_admissible, zk_authorization_queries_from_seeds, ZkAuthCapsuleOwnerProof,
    ZkAuthorizationProof, ZkAuthorizationUpper, ZK_AUTH_GRIND_BITS, ZK_AUTH_OWNER_PREFIX_CONSTANTS,
    ZK_AUTH_QUERY_SEEDS, ZK_AUTH_SOURCE_CAP_LANES,
};
use noid_gkr::zk_mlecheck::{
    combine_main_and_mask_round, mlecheck_endpoint_claim, ZkMleCheckRoundProof,
    ZkMleCheckVerifierState, ZK_MLECHECK_MASK_DEGREE,
};
use noid_poseidon2b::channel::Poseidon2bWideChannel;
use noid_poseidon2b::native::compression::Poseidon2bSponge;
use noid_poseidon2b::native::domain::{capacity_iv, DomainTag, TAG_ADDRFIX};
use noid_poseidon2b::native::permutation::{
    F_ROUNDS, MDS_FULL, MDS_PARTIAL, P_ROUNDS, ROUND_CONSTANTS,
};
use rand_core::OsRng;
use rayon::prelude::*;
use serde_json::{json, Value};

const PROGRAM_STEPS: usize = 8;
const PROGRAM_CELLS: usize = PROGRAM_STEPS * 2;
const PROGRAM_BYTES: usize = PROGRAM_CELLS * 16;
const STATE_LANES: usize = 4;
const STORED_ROWS: usize = ZK_AUTH_CAPSULE_STATE_LEN / STATE_LANES;
const POLICY_BASE_ROW: usize = 96;
const DUMMY_BOOLEAN_INDEX: usize = ZK_AUTH_CAPSULE_BANK_LEN - 1;
const MAIN_MAX_DEGREE: usize = 10;

const POLICY_STATEMENT_DOMAIN: DomainTag = DomainTag::new(b"POLICY__");
const POLICY_OWNER_PROTOCOL_TAG: u128 = 0x504F_4C49_4359_A001;
const POLICY_OWNER_CONSTRUCTION_VERSION: u128 = 1;
const POLICY_OWNER_TO_MAIN_CLOSE_TAG: u128 = 0x504F_4C49_4359_B001;
const POLICY_MAIN_FROM_OWNER_TAG: u128 = 0x504F_4C49_4359_AA01;
const POLICY_PHASE_B_TAG: u128 = 0x504F_4C49_4359_BA01;
const POLICY_MID_CAP_TAG: u128 = 0x504F_4C49_4359_AC01;
const POLICY_TAIL_TAG: u128 = 0x504F_4C49_4359_7A01;
const POLICY_GRIND_TAG: u128 = 0x504F_4C49_4359_6A01;

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
    immediate: u8,
}

impl Instruction {
    fn cells(self) -> [Block128; 2] {
        [
            Block128::from(self.opcode as u128),
            Block128::from(self.immediate as u128),
        ]
    }
}

#[derive(Clone, Debug)]
struct PolicyCase {
    name: &'static str,
    owner_secret: [Block128; 2],
    program: [Instruction; PROGRAM_STEPS],
    current: Block128,
    context: Block128,
    call_nonce: Block128,
}

impl PolicyCase {
    fn owner_permutation(&self) -> noid_gkr::PermLayerWitness {
        let iv = capacity_iv(TAG_ADDRFIX);
        evaluate_permutation([self.owner_secret[0], self.owner_secret[1], iv[0], iv[1]])
    }

    fn owner_address(&self) -> [Block128; 2] {
        let permutation = self.owner_permutation();
        [permutation.final_state()[0], permutation.final_state()[1]]
    }

    fn program_cells(&self) -> [[Block128; 2]; PROGRAM_STEPS] {
        self.program.map(Instruction::cells)
    }

    fn expected_next(&self) -> Block128 {
        execute(self.current, self.context, &self.program)
    }

    fn statement_digest(&self) -> [Block128; 2] {
        policy_statement_digest(
            self.program_cells(),
            self.current,
            self.expected_next(),
            self.context,
            self.call_nonce,
        )
    }

    fn boundary(&self) -> PolicyBoundary {
        PolicyBoundary {
            owner: AuthCapsuleBoundaryPublic::canonical(self.owner_address()),
            program: self.program_cells(),
            current: self.current,
            context: self.context,
            next: self.expected_next(),
        }
    }

    fn statement(&self) -> PolicyStatement {
        PolicyStatement {
            program: self.program_cells(),
            current: self.current,
            next: self.expected_next(),
            context: self.context,
            call_nonce: self.call_nonce,
            owner_address: self.owner_address(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PolicyBoundary {
    owner: AuthCapsuleBoundaryPublic,
    program: [[Block128; 2]; PROGRAM_STEPS],
    current: Block128,
    context: Block128,
    next: Block128,
}

/// Semantic policy statement. Five public policy values are compressed into
/// the first two lanes of the current four-lane authorization statement. The
/// final two lanes remain the expected private-owner address. The verifier
/// recomputes the digest before using every policy value in the boundary
/// relation, so `next`, `context`, and `call_nonce` are not free inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PolicyStatement {
    program: [[Block128; 2]; PROGRAM_STEPS],
    current: Block128,
    next: Block128,
    context: Block128,
    call_nonce: Block128,
    owner_address: [Block128; 2],
}

impl PolicyStatement {
    fn digest(self) -> [Block128; 2] {
        policy_statement_digest(
            self.program,
            self.current,
            self.next,
            self.context,
            self.call_nonce,
        )
    }

    fn transcript_fields(self) -> [Block128; 4] {
        let digest = self.digest();
        [
            digest[0],
            digest[1],
            self.owner_address[0],
            self.owner_address[1],
        ]
    }

    fn boundary(self) -> PolicyBoundary {
        PolicyBoundary {
            owner: AuthCapsuleBoundaryPublic::canonical(self.owner_address),
            program: self.program,
            current: self.current,
            context: self.context,
            next: self.next,
        }
    }
}

fn policy_statement_digest(
    program: [[Block128; 2]; PROGRAM_STEPS],
    current: Block128,
    next: Block128,
    context: Block128,
    call_nonce: Block128,
) -> [Block128; 2] {
    let mut sponge = Poseidon2bSponge::with_iv(capacity_iv(POLICY_STATEMENT_DOMAIN));
    for instruction in program {
        sponge.absorb_pair(instruction[0], instruction[1]);
    }
    sponge.absorb_pair(current, next);
    sponge.absorb_pair(context, call_nonce);
    let bytes = sponge.finalize();
    [
        Block128::from(u128::from_le_bytes(bytes[..16].try_into().unwrap())),
        Block128::from(u128::from_le_bytes(bytes[16..].try_into().unwrap())),
    ]
}

fn execute(
    mut accumulator: Block128,
    context: Block128,
    program: &[Instruction; PROGRAM_STEPS],
) -> Block128 {
    for instruction in program {
        let immediate = Block128::from(instruction.immediate as u128);
        accumulator = match instruction.opcode {
            Opcode::Nop => accumulator,
            Opcode::AddImmediate => accumulator + immediate,
            Opcode::MultiplyImmediate => accumulator * immediate,
            Opcode::SelectContextImmediate => accumulator + context * (accumulator + immediate),
        };
    }
    accumulator
}

fn state_index(row: usize, lane: usize) -> usize {
    assert!(row < STORED_ROWS && lane < STATE_LANES);
    row * STATE_LANES + lane
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

fn challenge(index: usize, domain: u128) -> Block256 {
    Block256::from_raw_challenge_lanes(
        deterministic_cell(2 * index, domain),
        deterministic_cell(2 * index + 1, domain ^ 0xA5A5_5A5A),
    )
}

fn build_state(case: &PolicyCase) -> Vec<Block128> {
    let mut state = vec![Block128::ZERO; ZK_AUTH_CAPSULE_STATE_LEN];

    let owner = case.owner_permutation();
    assert_eq!(owner.state.len(), ZK_AUTH_CAPSULE_ACTIVE_ROUNDS + 1);
    for (round, row) in owner.state.iter().enumerate() {
        let start = state_index(round, 0);
        state[start..start + STATE_LANES].copy_from_slice(row);
    }

    let mut accumulator = case.current;
    for (step, instruction) in case.program.iter().enumerate() {
        let instruction_cells = instruction.cells();
        let row = [
            accumulator,
            case.context,
            instruction_cells[0],
            instruction_cells[1],
        ];
        let start = state_index(POLICY_BASE_ROW + step, 0);
        state[start..start + STATE_LANES].copy_from_slice(&row);
        let immediate = Block128::from(instruction.immediate as u128);
        accumulator = match instruction.opcode {
            Opcode::Nop => accumulator,
            Opcode::AddImmediate => accumulator + immediate,
            Opcode::MultiplyImmediate => accumulator * immediate,
            Opcode::SelectContextImmediate => {
                accumulator + case.context * (accumulator + immediate)
            }
        };
    }
    let final_row = [accumulator, case.context, Block128::ZERO, Block128::ZERO];
    let start = state_index(POLICY_BASE_ROW + PROGRAM_STEPS, 0);
    state[start..start + STATE_LANES].copy_from_slice(&final_row);
    assert_eq!(accumulator, case.expected_next());
    state
}

fn build_bank(case: &PolicyCase) -> Vec<Block128> {
    let mut bank = vec![Block128::ZERO; ZK_AUTH_CAPSULE_BANK_LEN];
    bank[..ZK_AUTH_CAPSULE_STATE_LEN].copy_from_slice(&build_state(case));

    for (index, cell) in bank
        .iter_mut()
        .enumerate()
        .skip(ZK_AUTH_CAPSULE_LIBRA_MASK_OFFSET)
    {
        *cell = deterministic_cell(index, 0x5050_4F4C_4943_5901);
    }
    bank
}

#[derive(Clone)]
struct PolicyTables {
    increment: Vec<Block128>,
    lane: [Vec<Block128>; STATE_LANES],
    public: PolicyPublicTables,
}

#[derive(Clone)]
struct PolicyPublicTables {
    owner_active: Vec<Block128>,
    policy_state_active: Vec<Block128>,
    policy_context_active: Vec<Block128>,
    policy_opcode_active: Vec<Block128>,
    owner_mds: [Vec<Block128>; STATE_LANES],
    owner_sigma: [Vec<Block128>; STATE_LANES],
    owner_rc: [Vec<Block128>; STATE_LANES],
}

fn owner_is_partial_round(round: usize) -> bool {
    (F_ROUNDS / 2..F_ROUNDS / 2 + P_ROUNDS).contains(&round)
}

fn owner_sigma_at(round: usize, lane: usize) -> Block128 {
    if owner_is_partial_round(round) && lane != 0 {
        Block128::ZERO
    } else {
        Block128::ONE
    }
}

fn owner_round_constant_at(round: usize, lane: usize) -> Block128 {
    if owner_is_partial_round(round) && lane != 0 {
        Block128::ZERO
    } else {
        Block128::from(ROUND_CONSTANTS[lane][round])
    }
}

fn owner_mds_at(round: usize, output_lane: usize, input_lane: usize) -> Block128 {
    if owner_is_partial_round(round) {
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

fn lagrange4_base(opcode: Block128) -> ([Block128; 4], Block128) {
    let factors: [Block128; 4] =
        std::array::from_fn(|index| opcode + Block128::from(index as u128));
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
        numerators[selected] * denominator.invert()
    });
    (selectors, pair01 * pair23)
}

fn lagrange4_wide(opcode: Block256) -> ([Block256; 4], Block256) {
    let factors: [Block256; 4] =
        std::array::from_fn(|index| opcode + Block256::from(Block128::from(index as u128)));
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

fn universal_transition_base(
    state: Block128,
    context: Block128,
    opcode: Block128,
    immediate: Block128,
) -> (Block128, Block128) {
    let (selectors, validity) = lagrange4_base(opcode);
    let transition = selectors[0] * state
        + selectors[1] * (state + immediate)
        + selectors[2] * state * immediate
        + selectors[3] * (state + context * (state + immediate));
    (transition, validity)
}

fn universal_transition_wide(
    state: Block256,
    context: Block256,
    opcode: Block256,
    immediate: Block256,
) -> (Block256, Block256) {
    let (selectors, validity) = lagrange4_wide(opcode);
    let transition = selectors[0] * state
        + selectors[1] * (state + immediate)
        + selectors[2] * state * immediate
        + selectors[3] * (state + context * (state + immediate));
    (transition, validity)
}

impl PolicyPublicTables {
    fn build() -> Self {
        let mut owner_active = vec![Block128::ZERO; ZK_AUTH_CAPSULE_BANK_LEN];
        let mut policy_state_active = vec![Block128::ZERO; ZK_AUTH_CAPSULE_BANK_LEN];
        let mut policy_context_active = vec![Block128::ZERO; ZK_AUTH_CAPSULE_BANK_LEN];
        let mut policy_opcode_active = vec![Block128::ZERO; ZK_AUTH_CAPSULE_BANK_LEN];
        let mut owner_mds: [Vec<Block128>; STATE_LANES] =
            std::array::from_fn(|_| vec![Block128::ZERO; ZK_AUTH_CAPSULE_BANK_LEN]);
        let mut owner_sigma: [Vec<Block128>; STATE_LANES] =
            std::array::from_fn(|_| vec![Block128::ZERO; ZK_AUTH_CAPSULE_BANK_LEN]);
        let mut owner_rc: [Vec<Block128>; STATE_LANES] =
            std::array::from_fn(|_| vec![Block128::ZERO; ZK_AUTH_CAPSULE_BANK_LEN]);
        for index in 0..ZK_AUTH_CAPSULE_BANK_LEN {
            let high = index >> 9;
            let row = (index >> 2) & (STORED_ROWS - 1);
            let output_lane = index & (STATE_LANES - 1);
            if high != 0 {
                continue;
            }
            if row < ZK_AUTH_CAPSULE_ACTIVE_ROUNDS {
                owner_active[index] = Block128::ONE;
                for input_lane in 0..STATE_LANES {
                    owner_mds[input_lane][index] = owner_mds_at(row, output_lane, input_lane);
                    owner_sigma[input_lane][index] = owner_sigma_at(row, input_lane);
                    owner_rc[input_lane][index] = owner_round_constant_at(row, input_lane);
                }
                continue;
            }
            if !(POLICY_BASE_ROW..POLICY_BASE_ROW + PROGRAM_STEPS).contains(&row) {
                continue;
            }
            match output_lane {
                0 => policy_state_active[index] = Block128::ONE,
                1 => policy_context_active[index] = Block128::ONE,
                2 => policy_opcode_active[index] = Block128::ONE,
                _ => {}
            }
        }

        Self {
            owner_active,
            policy_state_active,
            policy_context_active,
            policy_opcode_active,
            owner_mds,
            owner_sigma,
            owner_rc,
        }
    }
}

impl PolicyTables {
    fn build(bank: ZkAuthCapsuleBankView<'_>) -> Self {
        let public = PolicyPublicTables::build();
        let mut increment = vec![Block128::ZERO; ZK_AUTH_CAPSULE_BANK_LEN];
        let mut lane: [Vec<Block128>; STATE_LANES] =
            std::array::from_fn(|_| vec![Block128::ZERO; ZK_AUTH_CAPSULE_BANK_LEN]);

        for index in 0..ZK_AUTH_CAPSULE_BANK_LEN {
            let high = index >> 9;
            let row = (index >> 2) & (STORED_ROWS - 1);
            let output_lane = index & (STATE_LANES - 1);
            let owner_active = row < ZK_AUTH_CAPSULE_ACTIVE_ROUNDS;
            let policy_active = (POLICY_BASE_ROW..POLICY_BASE_ROW + PROGRAM_STEPS).contains(&row);
            if high != 0 || (!owner_active && !policy_active) {
                continue;
            }
            if owner_active || output_lane < 2 {
                increment[index] = bank.state()[state_index(row + 1, output_lane)];
            }
            for input_lane in 0..STATE_LANES {
                if owner_active || output_lane < 3 {
                    lane[input_lane][index] = bank.state()[state_index(row, input_lane)];
                }
            }
        }

        increment[DUMMY_BOOLEAN_INDEX] = bank.cells()[ZK_AUTH_CAPSULE_TERMINAL_BLINDING_OFFSET];
        for input_lane in 0..STATE_LANES {
            lane[input_lane][DUMMY_BOOLEAN_INDEX] =
                bank.cells()[ZK_AUTH_CAPSULE_TERMINAL_BLINDING_OFFSET + 1 + input_lane];
        }

        Self {
            increment,
            lane,
            public,
        }
    }

    fn boolean_relation(&self, index: usize) -> Block128 {
        let mut owner = self.increment[index];
        for lane in 0..STATE_LANES {
            let state = self.lane[lane][index];
            let sigma = self.public.owner_sigma[lane][index];
            let with_rc = state + self.public.owner_rc[lane][index];
            let owner_pi = sigma * pow7_base(with_rc) + (Block128::ONE + sigma) * state;
            owner += self.public.owner_mds[lane][index] * owner_pi;
        }
        let (transition, opcode_validity) = universal_transition_base(
            self.lane[0][index],
            self.lane[1][index],
            self.lane[2][index],
            self.lane[3][index],
        );
        self.public.owner_active[index] * owner
            + self.public.policy_state_active[index] * (self.increment[index] + transition)
            + self.public.policy_context_active[index]
                * (self.increment[index] + self.lane[1][index])
            + self.public.policy_opcode_active[index] * opcode_validity
    }

    fn validate_boolean_relation(&self) -> bool {
        (0..ZK_AUTH_CAPSULE_BANK_LEN).all(|index| self.boolean_relation(index) == Block128::ZERO)
    }
}

#[derive(Clone, Copy)]
struct Polynomial {
    coefficients: [Block256; ZK_MLECHECK_MASK_DEGREE + 1],
    degree: usize,
}

impl Polynomial {
    fn zero() -> Self {
        Self {
            coefficients: [Block256::ZERO; ZK_MLECHECK_MASK_DEGREE + 1],
            degree: 0,
        }
    }

    fn affine(at_zero: Block256, at_one: Block256) -> Self {
        let mut result = Self::zero();
        result.coefficients[0] = at_zero;
        result.coefficients[1] = at_one - at_zero;
        result.degree = 1;
        result
    }

    fn one() -> Self {
        let mut result = Self::zero();
        result.coefficients[0] = Block256::ONE;
        result
    }

    fn constant(value: Block256) -> Self {
        let mut result = Self::zero();
        result.coefficients[0] = value;
        result
    }

    fn add_assign(&mut self, rhs: &Self) {
        for index in 0..=rhs.degree {
            self.coefficients[index] += rhs.coefficients[index];
        }
        self.degree = self.degree.max(rhs.degree);
    }

    fn add(mut self, rhs: &Self) -> Self {
        self.add_assign(rhs);
        self
    }

    fn mul(self, rhs: Self) -> Self {
        assert!(self.degree + rhs.degree <= ZK_MLECHECK_MASK_DEGREE);
        let mut result = Self::zero();
        for left in 0..=self.degree {
            for right in 0..=rhs.degree {
                result.coefficients[left + right] +=
                    self.coefficients[left] * rhs.coefficients[right];
            }
        }
        result.degree = self.degree + rhs.degree;
        result
    }

    fn pow7(self) -> Self {
        let square = self.mul(self);
        let fourth = square.mul(square);
        fourth.mul(square).mul(self)
    }

    fn scale(mut self, scalar: Block256) -> Self {
        for coefficient in &mut self.coefficients[..=self.degree] {
            *coefficient *= scalar;
        }
        self
    }

    fn into_round(self) -> RoundPolynomial<Block256> {
        RoundPolynomial::from_coeffs(self.coefficients.to_vec())
    }
}

fn lagrange4_polynomial(opcode: Polynomial) -> ([Polynomial; 4], Polynomial) {
    let factors: [Polynomial; 4] = std::array::from_fn(|index| {
        opcode.add(&Polynomial::constant(Block256::from(Block128::from(
            index as u128,
        ))))
    });
    let pair01 = factors[0].mul(factors[1]);
    let pair23 = factors[2].mul(factors[3]);
    let numerators = [
        factors[1].mul(pair23),
        factors[0].mul(pair23),
        pair01.mul(factors[3]),
        pair01.mul(factors[2]),
    ];
    let selectors = std::array::from_fn(|selected| {
        let selected_value = Block128::from(selected as u128);
        let denominator = (0..4)
            .filter(|other| *other != selected)
            .fold(Block128::ONE, |acc, other| {
                acc * (selected_value + Block128::from(other as u128))
            });
        numerators[selected].scale(Block256::from(denominator.invert()))
    });
    (selectors, pair01.mul(pair23))
}

fn universal_transition_polynomial(
    state: Polynomial,
    context: Polynomial,
    opcode: Polynomial,
    immediate: Polynomial,
) -> (Polynomial, Polynomial) {
    let (selectors, validity) = lagrange4_polynomial(opcode);
    let cases = [
        state,
        state.add(&immediate),
        state.mul(immediate),
        state.add(&context.mul(state.add(&immediate))),
    ];
    let mut transition = Polynomial::zero();
    for index in 0..4 {
        transition.add_assign(&selectors[index].mul(cases[index]));
    }
    (transition, validity)
}

fn restrict_high(table: &[Block128], prior: &[Block256]) -> Vec<Block256> {
    let mut values: Vec<_> = table.iter().copied().map(Block256::from).collect();
    for &challenge in prior {
        let half = values.len() / 2;
        for index in 0..half {
            let at_zero = values[index];
            let at_one = values[index + half];
            values[index] = at_zero + challenge * (at_one - at_zero);
        }
        values.truncate(half);
    }
    values
}

fn endpoint(table: &[Block256], lower_index: usize, half: usize) -> Polynomial {
    Polynomial::affine(table[lower_index], table[lower_index + half])
}

fn boolean_eq_weight(point: &[Block256], boolean_index: usize) -> Block256 {
    point
        .iter()
        .enumerate()
        .fold(Block256::ONE, |weight, (variable, &coordinate)| {
            if ((boolean_index >> variable) & 1) == 0 {
                weight * (Block256::ONE - coordinate)
            } else {
                weight * coordinate
            }
        })
}

fn main_round(
    tables: &PolicyTables,
    input_point: &[Block256; ZK_AUTH_CAPSULE_BANK_VARS],
    prior: &[Block256],
) -> RoundPolynomial<Block256> {
    assert!(prior.len() < ZK_AUTH_CAPSULE_BANK_VARS);
    let current_variable = ZK_AUTH_CAPSULE_BANK_VARS - 1 - prior.len();
    let owner_active = restrict_high(&tables.public.owner_active, prior);
    let policy_state_active = restrict_high(&tables.public.policy_state_active, prior);
    let policy_context_active = restrict_high(&tables.public.policy_context_active, prior);
    let policy_opcode_active = restrict_high(&tables.public.policy_opcode_active, prior);
    let increment = restrict_high(&tables.increment, prior);
    let lane: [Vec<Block256>; STATE_LANES] =
        std::array::from_fn(|index| restrict_high(&tables.lane[index], prior));
    let owner_mds: [Vec<Block256>; STATE_LANES] =
        std::array::from_fn(|index| restrict_high(&tables.public.owner_mds[index], prior));
    let owner_sigma: [Vec<Block256>; STATE_LANES] =
        std::array::from_fn(|index| restrict_high(&tables.public.owner_sigma[index], prior));
    let owner_rc: [Vec<Block256>; STATE_LANES] =
        std::array::from_fn(|index| restrict_high(&tables.public.owner_rc[index], prior));
    let half = 1usize << current_variable;
    assert_eq!(owner_active.len(), 2 * half);

    let mut round = Polynomial::zero();
    for lower_index in 0..half {
        let increment = endpoint(&increment, lower_index, half);
        let lanes: [Polynomial; STATE_LANES] =
            std::array::from_fn(|lane_index| endpoint(&lane[lane_index], lower_index, half));
        let mut owner = increment;
        for input_lane in 0..STATE_LANES {
            let state = lanes[input_lane];
            let mds = endpoint(&owner_mds[input_lane], lower_index, half);
            let sigma = endpoint(&owner_sigma[input_lane], lower_index, half);
            let rc = endpoint(&owner_rc[input_lane], lower_index, half);
            let owner_pi = sigma
                .mul(state.add(&rc).pow7())
                .add(&Polynomial::one().add(&sigma).mul(state));
            owner.add_assign(&mds.mul(owner_pi));
        }
        let (transition, opcode_validity) =
            universal_transition_polynomial(lanes[0], lanes[1], lanes[2], lanes[3]);
        let mut relation = endpoint(&owner_active, lower_index, half).mul(owner);
        relation.add_assign(
            &endpoint(&policy_state_active, lower_index, half).mul(increment.add(&transition)),
        );
        relation.add_assign(
            &endpoint(&policy_context_active, lower_index, half).mul(increment.add(&lanes[1])),
        );
        relation
            .add_assign(&endpoint(&policy_opcode_active, lower_index, half).mul(opcode_validity));
        let weighted = relation.scale(boolean_eq_weight(
            &input_point[..current_variable],
            lower_index,
        ));
        round.add_assign(&weighted);
    }
    assert!(round.degree <= MAIN_MAX_DEGREE);
    round.into_round()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TerminalOperands {
    increment: Block256,
    lane: [Block256; STATE_LANES],
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

fn terminal_operands(
    tables: &PolicyTables,
    point: &[Block256; ZK_AUTH_CAPSULE_BANK_VARS],
) -> TerminalOperands {
    TerminalOperands {
        increment: evaluate_mle_low_to_high(&tables.increment, point).unwrap(),
        lane: std::array::from_fn(|lane| {
            evaluate_mle_low_to_high(&tables.lane[lane], point).unwrap()
        }),
    }
}

fn terminal_main(
    tables: &PolicyTables,
    point: &[Block256; ZK_AUTH_CAPSULE_BANK_VARS],
    operands: TerminalOperands,
) -> Block256 {
    terminal_main_from_public(&tables.public, point, operands)
}

fn terminal_main_from_public(
    tables: &PolicyPublicTables,
    point: &[Block256; ZK_AUTH_CAPSULE_BANK_VARS],
    operands: TerminalOperands,
) -> Block256 {
    let mut owner = operands.increment;
    for lane in 0..STATE_LANES {
        let state = operands.lane[lane];
        let mds = evaluate_mle_low_to_high(&tables.owner_mds[lane], point).unwrap();
        let sigma = evaluate_mle_low_to_high(&tables.owner_sigma[lane], point).unwrap();
        let rc = evaluate_mle_low_to_high(&tables.owner_rc[lane], point).unwrap();
        owner += mds * (sigma * pow7_wide(state + rc) + (Block256::ONE + sigma) * state);
    }
    let (transition, opcode_validity) = universal_transition_wide(
        operands.lane[0],
        operands.lane[1],
        operands.lane[2],
        operands.lane[3],
    );
    evaluate_mle_low_to_high(&tables.owner_active, point).unwrap() * owner
        + evaluate_mle_low_to_high(&tables.policy_state_active, point).unwrap()
            * (operands.increment + transition)
        + evaluate_mle_low_to_high(&tables.policy_context_active, point).unwrap()
            * (operands.increment + operands.lane[1])
        + evaluate_mle_low_to_high(&tables.policy_opcode_active, point).unwrap() * opcode_validity
}

fn terminal_functional_weights(
    point: &[Block256; ZK_AUTH_CAPSULE_BANK_VARS],
) -> [Vec<Block256>; 5] {
    let eq = mle_weights_low_to_high(point);
    let mut output: [Vec<Block256>; 5] =
        std::array::from_fn(|_| vec![Block256::ZERO; ZK_AUTH_CAPSULE_BANK_LEN]);
    for index in 0..ZK_AUTH_CAPSULE_BANK_LEN {
        let high = index >> 9;
        let row = (index >> 2) & (STORED_ROWS - 1);
        let output_lane = index & (STATE_LANES - 1);
        let owner_active = row < ZK_AUTH_CAPSULE_ACTIVE_ROUNDS;
        let policy_active = (POLICY_BASE_ROW..POLICY_BASE_ROW + PROGRAM_STEPS).contains(&row);
        if high == 0 && (owner_active || policy_active) {
            if owner_active || output_lane < 2 {
                output[0][state_index(row + 1, output_lane)] += eq[index];
            }
            if owner_active || output_lane < 3 {
                for input_lane in 0..STATE_LANES {
                    output[1 + input_lane][state_index(row, input_lane)] += eq[index];
                }
            }
        }
    }
    output[0][ZK_AUTH_CAPSULE_TERMINAL_BLINDING_OFFSET] += eq[DUMMY_BOOLEAN_INDEX];
    for lane in 0..STATE_LANES {
        output[1 + lane][ZK_AUTH_CAPSULE_TERMINAL_BLINDING_OFFSET + 1 + lane] +=
            eq[DUMMY_BOOLEAN_INDEX];
    }
    output
}

fn inner_product(bank: &[Block128], weights: &[Block256]) -> Block256 {
    bank.iter()
        .copied()
        .zip(weights)
        .fold(Block256::ZERO, |acc, (cell, weight)| {
            acc + Block256::from(cell) * *weight
        })
}

fn build_policy_post_claim_relation(
    input_point: &[Block256; ZK_AUTH_CAPSULE_BANK_VARS],
    terminal_point: &[Block256; ZK_AUTH_CAPSULE_BANK_VARS],
    operands: TerminalOperands,
    mask_at_input: Block256,
    mask_at_terminal: Block256,
    boundary: PolicyBoundary,
    eta: Block256,
) -> AuthCapsulePostClaimRelation<Block256> {
    assert_ne!(eta, Block256::ZERO);
    let terminal_weights = terminal_functional_weights(terminal_point);
    let mask_input_weights = libra_mask_mle_functional_weights(input_point);
    let mask_terminal_weights = libra_mask_final_functional_weights(terminal_point);
    let mut weights = vec![Block256::ZERO; ZK_AUTH_CAPSULE_BANK_LEN];
    let mut expected = Block256::ZERO;
    let mut power = Block256::ONE;

    let mut accumulate = |source: &[Block256], value: Block256| {
        for (target, source) in weights.iter_mut().zip(source) {
            *target += power * *source;
        }
        expected += power * value;
        power *= eta;
    };

    for (source, value) in terminal_weights.iter().zip(operands.ordered()) {
        accumulate(source, value);
    }
    for claim in sparse_boundary_claims(boundary.owner) {
        let mut source = vec![Block256::ZERO; ZK_AUTH_CAPSULE_BANK_LEN];
        for term in claim.terms {
            source[term.bank_index] += Block256::from(term.coefficient);
        }
        accumulate(&source, Block256::from(claim.expected));
    }
    for (step, instruction) in boundary.program.iter().enumerate() {
        let mut source = vec![Block256::ZERO; ZK_AUTH_CAPSULE_BANK_LEN];
        source[state_index(POLICY_BASE_ROW + step, 2)] = Block256::ONE;
        accumulate(&source, Block256::from(instruction[0]));
        let mut source = vec![Block256::ZERO; ZK_AUTH_CAPSULE_BANK_LEN];
        source[state_index(POLICY_BASE_ROW + step, 3)] = Block256::ONE;
        accumulate(&source, Block256::from(instruction[1]));
    }
    let mut source = vec![Block256::ZERO; ZK_AUTH_CAPSULE_BANK_LEN];
    source[state_index(POLICY_BASE_ROW, 0)] = Block256::ONE;
    accumulate(&source, Block256::from(boundary.current));
    let mut source = vec![Block256::ZERO; ZK_AUTH_CAPSULE_BANK_LEN];
    source[state_index(POLICY_BASE_ROW, 1)] = Block256::ONE;
    accumulate(&source, Block256::from(boundary.context));
    let mut source = vec![Block256::ZERO; ZK_AUTH_CAPSULE_BANK_LEN];
    source[state_index(POLICY_BASE_ROW + PROGRAM_STEPS, 0)] = Block256::ONE;
    accumulate(&source, Block256::from(boundary.next));
    accumulate(&mask_input_weights, mask_at_input);
    accumulate(&mask_terminal_weights, mask_at_terminal);

    let relation = AuthCapsulePostClaimRelation {
        weights,
        expected_inner_product: expected,
    };
    relation
}

fn max_nonzero_degree(round: &RoundPolynomial<Block256>) -> usize {
    round
        .coeffs
        .iter()
        .rposition(|coefficient| *coefficient != Block256::ZERO)
        .unwrap_or(0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PolicyOwnerDerived {
    rho: [Block256; ZK_AUTH_CAPSULE_BANK_VARS],
    lambda: Block256,
    round_challenges_high_to_low: [Block256; ZK_AUTH_CAPSULE_BANK_VARS],
    terminal_point: [Block256; ZK_AUTH_CAPSULE_BANK_VARS],
    terminal_operands: TerminalOperands,
    mask_mu: Block256,
    mask_final: Block256,
    main_final: Block256,
    eta: Block256,
    post_claim_relation: AuthCapsulePostClaimRelation<Block256>,
    bridge: [Block128; 4],
}

impl PolicyOwnerDerived {
    fn bank_claim(&self) -> Block256 {
        self.post_claim_relation.expected_inner_product
    }
}

#[derive(Clone, Debug)]
struct PolicyOwnerProverOutput {
    proof: ZkAuthCapsuleOwnerProof,
    derived: PolicyOwnerDerived,
}

#[derive(Clone, Debug)]
struct PolicyAuthorizationVerified {
    owner: PolicyOwnerDerived,
    gamma: Block256,
    queries: Vec<usize>,
}

fn err<T: std::fmt::Debug>(value: T) -> String {
    format!("{value:?}")
}

fn absorb_policy_owner_prefix(
    channel: &mut Poseidon2bWideChannel,
    statement: PolicyStatement,
    source_cap: &[Block128; ZK_AUTH_SOURCE_CAP_LANES],
) {
    let mut constants = ZK_AUTH_OWNER_PREFIX_CONSTANTS;
    constants[0] = POLICY_OWNER_PROTOCOL_TAG;
    constants[1] = POLICY_OWNER_CONSTRUCTION_VERSION;
    for constant in constants {
        channel.absorb_base(Block128::from(constant));
    }
    channel.absorb_base_slice(&statement.transcript_fields());
    channel.absorb_base_slice(source_cap);
}

fn absorb_policy_owner_round(
    channel: &mut Poseidon2bWideChannel,
    round: &ZkMleCheckRoundProof<Block256>,
) {
    for coefficient in round.coeffs_without_constant {
        channel.absorb_wide(coefficient);
    }
}

fn absorb_policy_owner_terminal(
    channel: &mut Poseidon2bWideChannel,
    mask_final: Block256,
    terminal_operands: TerminalOperands,
) {
    channel.absorb_wide(mask_final);
    for claim in terminal_operands.ordered() {
        channel.absorb_wide(claim);
    }
}

fn validate_policy_boundary(
    bank: ZkAuthCapsuleBankView<'_>,
    boundary: PolicyBoundary,
) -> Result<(), String> {
    validate_sparse_boundary(bank, boundary.owner).map_err(err)?;
    for (step, instruction) in boundary.program.iter().enumerate() {
        for code_lane in 0..2 {
            let actual = bank.state()[state_index(POLICY_BASE_ROW + step, code_lane + 2)];
            if actual != instruction[code_lane] {
                return Err(format!(
                    "policy program boundary mismatch at step {step}, lane {code_lane}"
                ));
            }
        }
    }
    if bank.state()[state_index(POLICY_BASE_ROW, 0)] != boundary.current {
        return Err("policy current State boundary mismatch".to_owned());
    }
    if bank.state()[state_index(POLICY_BASE_ROW, 1)] != boundary.context {
        return Err("policy context boundary mismatch".to_owned());
    }
    if bank.state()[state_index(POLICY_BASE_ROW + PROGRAM_STEPS, 0)] != boundary.next {
        return Err("policy next State boundary mismatch".to_owned());
    }
    Ok(())
}

fn prove_policy_owner(
    bank: ZkAuthCapsuleBankView<'_>,
    statement: PolicyStatement,
    source_cap: &[Block128; ZK_AUTH_SOURCE_CAP_LANES],
) -> Result<PolicyOwnerProverOutput, String> {
    let tables = PolicyTables::build(bank);
    if !tables.validate_boolean_relation() {
        return Err("policy transition relation is false on the Boolean cube".to_owned());
    }
    validate_policy_boundary(bank, statement.boundary())?;

    let mask = bank.libra_mask_view().map_err(err)?;
    let mut channel = Poseidon2bWideChannel::new();
    absorb_policy_owner_prefix(&mut channel, statement, source_cap);
    let rho = std::array::from_fn(|_| channel.squeeze_wide());

    let mask_mu = mask.evaluate_mle(&rho);
    channel.absorb_wide(mask_mu);
    let lambda = channel.squeeze_wide();
    if lambda == Block256::ZERO {
        return Err("zero policy lambda".to_owned());
    }

    let mut prior_challenges = Vec::with_capacity(ZK_AUTH_CAPSULE_BANK_VARS);
    let mut round_proofs = Vec::with_capacity(ZK_AUTH_CAPSULE_BANK_VARS);
    let mut main_running_claim = Block256::ZERO;
    for round_index in 0..ZK_AUTH_CAPSULE_BANK_VARS {
        let variable = ZK_AUTH_CAPSULE_BANK_VARS - 1 - round_index;
        let main = main_round(&tables, &rho, &prior_challenges);
        if mlecheck_endpoint_claim(&main.coeffs, rho[variable]) != main_running_claim {
            return Err(format!("policy endpoint mismatch at round {round_index}"));
        }
        let mask_round = mask
            .round_coefficients(&rho, &prior_challenges)
            .map_err(err)?;
        let combined = combine_main_and_mask_round(&main, &mask_round, lambda).map_err(err)?;
        let round_proof = ZkMleCheckRoundProof::truncate(&combined).map_err(err)?;
        absorb_policy_owner_round(&mut channel, &round_proof);
        let challenge = channel.squeeze_wide();
        main_running_claim = main.evaluate(challenge);
        prior_challenges.push(challenge);
        round_proofs.push(round_proof);
    }

    let round_challenges_high_to_low: [Block256; ZK_AUTH_CAPSULE_BANK_VARS] = prior_challenges
        .try_into()
        .map_err(|_| "wrong policy round count".to_owned())?;
    let terminal_point = std::array::from_fn(|variable| {
        round_challenges_high_to_low[ZK_AUTH_CAPSULE_BANK_VARS - 1 - variable]
    });
    certify_terminal_blinding_rank(&terminal_point).map_err(err)?;
    let terminal_operands = terminal_operands(&tables, &terminal_point);
    let main_final = terminal_main(&tables, &terminal_point, terminal_operands);
    let public_tables = PolicyPublicTables::build();
    let public_main_final =
        terminal_main_from_public(&public_tables, &terminal_point, terminal_operands);
    if main_running_claim != main_final || main_final != public_main_final {
        return Err("policy terminal mismatch".to_owned());
    }
    let mask_final = mask.evaluate_final(&terminal_point);
    absorb_policy_owner_terminal(&mut channel, mask_final, terminal_operands);
    let eta = channel.squeeze_wide();
    if eta == Block256::ZERO {
        return Err("zero policy eta".to_owned());
    }

    let post_claim_relation = build_policy_post_claim_relation(
        &rho,
        &terminal_point,
        terminal_operands,
        mask_mu,
        mask_final,
        statement.boundary(),
        eta,
    );
    if !post_claim_relation.verify(bank) {
        return Err("policy post-claim relation mismatch".to_owned());
    }
    let bridge = channel.close_into_bridge(Block128::from(POLICY_OWNER_TO_MAIN_CLOSE_TAG));

    let proof = ZkAuthCapsuleOwnerProof {
        mask_mu,
        rounds: round_proofs
            .try_into()
            .map_err(|_| "wrong policy proof round count".to_owned())?,
        mask_final,
        terminal_operand_claims: terminal_operands.ordered(),
    };
    let derived = PolicyOwnerDerived {
        rho,
        lambda,
        round_challenges_high_to_low,
        terminal_point,
        terminal_operands,
        mask_mu,
        mask_final,
        main_final,
        eta,
        post_claim_relation,
        bridge,
    };
    let replayed = verify_policy_owner(statement, source_cap, &proof)?;
    if replayed != derived {
        return Err("policy prover/verifier differential mismatch".to_owned());
    }
    Ok(PolicyOwnerProverOutput { proof, derived })
}

fn verify_policy_owner(
    statement: PolicyStatement,
    source_cap: &[Block128; ZK_AUTH_SOURCE_CAP_LANES],
    proof: &ZkAuthCapsuleOwnerProof,
) -> Result<PolicyOwnerDerived, String> {
    let mut channel = Poseidon2bWideChannel::new();
    absorb_policy_owner_prefix(&mut channel, statement, source_cap);
    let rho = std::array::from_fn(|_| channel.squeeze_wide());

    channel.absorb_wide(proof.mask_mu);
    let lambda = channel.squeeze_wide();
    if lambda == Block256::ZERO {
        return Err("zero policy lambda".to_owned());
    }
    let mut verifier = ZkMleCheckVerifierState::new(rho, Block256::ZERO, proof.mask_mu, lambda);
    let mut round_challenges_high_to_low = [Block256::ZERO; ZK_AUTH_CAPSULE_BANK_VARS];
    for (round_index, round) in proof.rounds.iter().enumerate() {
        absorb_policy_owner_round(&mut channel, round);
        let challenge = channel.squeeze_wide();
        round_challenges_high_to_low[round_index] = challenge;
        verifier.transition(round, challenge).map_err(err)?;
    }

    let terminal_point = std::array::from_fn(|variable| {
        round_challenges_high_to_low[ZK_AUTH_CAPSULE_BANK_VARS - 1 - variable]
    });
    certify_terminal_blinding_rank(&terminal_point).map_err(err)?;
    let terminal_operands = TerminalOperands {
        increment: proof.terminal_operand_claims[0],
        lane: std::array::from_fn(|lane| proof.terminal_operand_claims[1 + lane]),
    };
    let public_tables = PolicyPublicTables::build();
    let main_final = terminal_main_from_public(&public_tables, &terminal_point, terminal_operands);
    verifier
        .finish_checked(proof.mask_final, main_final)
        .map_err(err)?;

    absorb_policy_owner_terminal(&mut channel, proof.mask_final, terminal_operands);
    let eta = channel.squeeze_wide();
    if eta == Block256::ZERO {
        return Err("zero policy eta".to_owned());
    }
    let post_claim_relation = build_policy_post_claim_relation(
        &rho,
        &terminal_point,
        terminal_operands,
        proof.mask_mu,
        proof.mask_final,
        statement.boundary(),
        eta,
    );
    let bridge = channel.close_into_bridge(Block128::from(POLICY_OWNER_TO_MAIN_CLOSE_TAG));

    Ok(PolicyOwnerDerived {
        rho,
        lambda,
        round_challenges_high_to_low,
        terminal_point,
        terminal_operands,
        mask_mu: proof.mask_mu,
        mask_final: proof.mask_final,
        main_final,
        eta,
        post_claim_relation,
        bridge,
    })
}

fn init_policy_main_channel(owner: &PolicyOwnerDerived, sigma: Block256) -> Poseidon2bWideChannel {
    let mut channel = Poseidon2bWideChannel::new();
    channel.absorb_base(Block128::from(POLICY_MAIN_FROM_OWNER_TAG));
    channel.absorb_base_slice(&owner.bridge);
    channel.absorb_wide(sigma);
    channel
}

fn absorb_policy_phase_a_round(
    channel: &mut Poseidon2bWideChannel,
    round: &noid_fri_binius::ZkPhaseARoundProof<Block256>,
) {
    channel.absorb_wide(round.at_one);
    channel.absorb_wide(round.at_infinity);
}

fn absorb_policy_phase_b_prefix(
    channel: &mut Poseidon2bWideChannel,
    phase_b_value: Block256,
    upper: &ZkAuthorizationUpper,
) {
    channel.absorb_base(Block128::from(POLICY_PHASE_B_TAG));
    channel.absorb_wide(phase_b_value);
    for value in upper.as_array() {
        channel.absorb_wide(*value);
    }
}

fn absorb_policy_mid_commitment(
    channel: &mut Poseidon2bWideChannel,
    mid: &ZkCapsulePcsMidCommitment,
) -> Result<(), String> {
    channel.absorb_base(Block128::from(POLICY_MID_CAP_TAG));
    channel.absorb_base_slice(&mid.transcript_lanes().map_err(err)?);
    Ok(())
}

fn absorb_policy_tail(channel: &mut Poseidon2bWideChannel, tail: &ZkCapsulePcsTailReveal) {
    channel.absorb_base(Block128::from(POLICY_TAIL_TAG));
    for coefficient in tail.coefficients {
        channel.absorb_wide(coefficient);
    }
}

fn verify_policy_phase_b_upper_tail_link(
    upper: &[Block256; 256],
    phase_a_terminal_point: &[Block256; PHASE_A_VARS],
    phase_b_value: Block256,
    beta: &[Block256; PHASE_B_LOW_VARS],
    tail: &[Block256; TAIL_SYMBOLS],
) -> Result<(), String> {
    let low: &[Block256; PHASE_B_LOW_VARS] = phase_a_terminal_point[..PHASE_B_LOW_VARS]
        .try_into()
        .map_err(|_| "wrong low Phase-B point length".to_owned())?;
    if evaluate_upper_at_low8(upper, low) != phase_b_value {
        return Err("Phase-B terminal value mismatch".to_owned());
    }
    let upper_at_beta = evaluate_upper_at_low8(upper, beta);
    let h: [Block256; FINAL_H_SYMBOLS] = tail16_local_fold(tail, beta[PHASE_B_LOW_VARS - 1]);
    let high: &[Block256; PHASE_B_HIGH_VARS] = phase_a_terminal_point[PHASE_B_LOW_VARS..]
        .try_into()
        .map_err(|_| "wrong high Phase-B point length".to_owned())?;
    if upper_at_beta != evaluate_slice(&h, high) {
        return Err("Phase-B upper/tail mismatch".to_owned());
    }
    Ok(())
}

fn policy_grind_probe(channel: &Poseidon2bWideChannel, nonce: u64) -> Block128 {
    let mut probe = channel.clone();
    probe.absorb_base(Block128::from(nonce as u128));
    probe.squeeze_base()
}

fn policy_grind(channel: &mut Poseidon2bWideChannel) -> Result<(u64, Block128), String> {
    channel.absorb_base(Block128::from(POLICY_GRIND_TAG));
    let mask = (1u128 << ZK_AUTH_GRIND_BITS) - 1;
    let block = 1u64 << (ZK_AUTH_GRIND_BITS + 1);
    let mut start = 0u64;
    let nonce = loop {
        let end = start.saturating_add(block);
        if let Some(found) = (start..end)
            .into_par_iter()
            .find_first(|nonce| policy_grind_probe(channel, *nonce).0 & mask == 0)
        {
            break found;
        }
        if end == u64::MAX {
            return Err("policy grind exhausted".to_owned());
        }
        start = end;
    };
    channel.absorb_base(Block128::from(nonce as u128));
    let grind = channel.squeeze_base();
    if grind.0 & mask != 0 {
        return Err("policy grind rejected".to_owned());
    }
    Ok((nonce, grind))
}

fn replay_policy_grind(
    channel: &mut Poseidon2bWideChannel,
    nonce: u64,
) -> Result<Block128, String> {
    channel.absorb_base(Block128::from(POLICY_GRIND_TAG));
    channel.absorb_base(Block128::from(nonce as u128));
    let grind = channel.squeeze_base();
    if grind.0 & ((1u128 << ZK_AUTH_GRIND_BITS) - 1) != 0 {
        return Err("policy grind rejected".to_owned());
    }
    Ok(grind)
}

fn prove_policy_authorization(case: &PolicyCase) -> Result<ZkAuthorizationProof, String> {
    let statement = case.statement();
    let state = build_state(case);
    let (source_commitment, source_state) =
        zk_capsule_pcs_commit_fresh(&state, &mut OsRng).map_err(err)?;
    let source_cap = source_commitment.transcript_lanes().map_err(err)?;
    let (owner_output, owner_bound) = zk_capsule_pcs_bind_owner(source_state, |bank| {
        let bank = ZkAuthCapsuleBankView::checked(bank).map_err(err)?;
        prove_policy_owner(bank, statement, &source_cap)
    })?;

    let (phase_a_binding, phase_a_bound) = zk_capsule_pcs_bind_phase_a(
        owner_bound,
        &owner_output.derived.post_claim_relation.weights,
        owner_output.derived.bank_claim(),
    )
    .map_err(err)?;
    let sigma = phase_a_binding.companion_claim;
    let mut channel = init_policy_main_channel(&owner_output.derived, sigma);
    let gamma = channel.squeeze_wide();
    if !affine_blend_gamma_is_admissible(gamma) {
        return Err("inadmissible policy gamma".to_owned());
    }

    let (phase_a_output, phase_a_complete) =
        zk_capsule_pcs_prove_phase_a(phase_a_bound, gamma, |_round, round_proof| {
            absorb_policy_phase_a_round(&mut channel, &round_proof);
            channel.squeeze_wide()
        })
        .map_err(err)?;
    if phase_a_output.relation_claims.bank != owner_output.derived.bank_claim()
        || phase_a_output.relation_claims.companion != sigma
    {
        return Err("policy Phase-A binding mismatch".to_owned());
    }

    let phase_b_value = phase_a_output.terminal_oracle_value;
    let (phase_b_link, phase_b_ready) =
        zk_capsule_pcs_link_phase_b(phase_a_complete, phase_b_value).map_err(err)?;
    let upper = ZkAuthorizationUpper::new(phase_b_link.upper);
    absorb_policy_phase_b_prefix(&mut channel, phase_b_value, &upper);
    let beta_source: [Block256; SOURCE_STANDARD_FOLDS] =
        std::array::from_fn(|_| channel.squeeze_wide());

    let (mid_commitment, mid_state) =
        zk_capsule_pcs_commit_mid(phase_b_ready, beta_source).map_err(err)?;
    absorb_policy_mid_commitment(&mut channel, &mid_commitment)?;
    let beta_mid: [Block256; MID_STANDARD_FOLDS] = std::array::from_fn(|_| channel.squeeze_wide());

    let tail_state = zk_capsule_pcs_reveal_tail(mid_state, beta_mid).map_err(err)?;
    let tail = tail_state.tail.clone();
    absorb_policy_tail(&mut channel, &tail);
    let beta_tail = channel.squeeze_wide();
    let mut beta = [Block256::ZERO; PHASE_B_LOW_VARS];
    beta[..SOURCE_STANDARD_FOLDS].copy_from_slice(&beta_source);
    beta[SOURCE_STANDARD_FOLDS..SOURCE_STANDARD_FOLDS + MID_STANDARD_FOLDS]
        .copy_from_slice(&beta_mid);
    beta[PHASE_B_LOW_VARS - 1] = beta_tail;
    verify_policy_phase_b_upper_tail_link(
        upper.as_array(),
        &phase_a_output.terminal_point,
        phase_b_value,
        &beta,
        &tail.coefficients,
    )?;

    let (grind_nonce, _grind) = policy_grind(&mut channel)?;
    let query_seeds: [Block128; ZK_AUTH_QUERY_SEEDS] =
        std::array::from_fn(|_| channel.squeeze_base());
    let queries = zk_authorization_queries_from_seeds(&query_seeds);
    let opening = zk_capsule_pcs_open(tail_state, &queries).map_err(err)?;
    let proof = ZkAuthorizationProof {
        source_commitment,
        owner: owner_output.proof,
        sigma,
        phase_a: phase_a_output.proof,
        phase_b_value,
        upper,
        mid_commitment,
        tail,
        grind_nonce,
        opening,
    };
    proof.preflight_shape().map_err(err)?;
    verify_policy_authorization(statement, &proof)?;
    Ok(proof)
}

fn verify_policy_authorization(
    statement: PolicyStatement,
    proof: &ZkAuthorizationProof,
) -> Result<PolicyAuthorizationVerified, String> {
    proof.preflight_shape().map_err(err)?;
    let source_cap = proof.source_commitment.transcript_lanes().map_err(err)?;
    let owner = verify_policy_owner(statement, &source_cap, &proof.owner)?;

    let mut channel = init_policy_main_channel(&owner, proof.sigma);
    let gamma = channel.squeeze_wide();
    if !affine_blend_gamma_is_admissible(gamma) {
        return Err("inadmissible policy gamma".to_owned());
    }
    let mut phase_a_challenges_high_to_low = [Block256::ZERO; PHASE_A_VARS];
    for (index, round) in proof.phase_a.rounds.iter().enumerate() {
        absorb_policy_phase_a_round(&mut channel, round);
        phase_a_challenges_high_to_low[index] = channel.squeeze_wide();
    }
    let phase_a = verify_phase_a(
        &proof.phase_a,
        ZkPhaseARelationClaims {
            bank: owner.bank_claim(),
            companion: proof.sigma,
        },
        &owner.post_claim_relation.weights,
        gamma,
        &phase_a_challenges_high_to_low,
        proof.phase_b_value,
    )
    .map_err(err)?;

    absorb_policy_phase_b_prefix(&mut channel, proof.phase_b_value, &proof.upper);
    let beta_source: [Block256; SOURCE_STANDARD_FOLDS] =
        std::array::from_fn(|_| channel.squeeze_wide());
    absorb_policy_mid_commitment(&mut channel, &proof.mid_commitment)?;
    let beta_mid: [Block256; MID_STANDARD_FOLDS] = std::array::from_fn(|_| channel.squeeze_wide());
    absorb_policy_tail(&mut channel, &proof.tail);
    let beta_tail = channel.squeeze_wide();
    let mut beta = [Block256::ZERO; PHASE_B_LOW_VARS];
    beta[..SOURCE_STANDARD_FOLDS].copy_from_slice(&beta_source);
    beta[SOURCE_STANDARD_FOLDS..SOURCE_STANDARD_FOLDS + MID_STANDARD_FOLDS]
        .copy_from_slice(&beta_mid);
    beta[PHASE_B_LOW_VARS - 1] = beta_tail;
    verify_policy_phase_b_upper_tail_link(
        proof.upper.as_array(),
        &phase_a.terminal_point,
        proof.phase_b_value,
        &beta,
        &proof.tail.coefficients,
    )?;

    let _grind = replay_policy_grind(&mut channel, proof.grind_nonce)?;
    let query_seeds: [Block128; ZK_AUTH_QUERY_SEEDS] =
        std::array::from_fn(|_| channel.squeeze_base());
    let queries = zk_authorization_queries_from_seeds(&query_seeds);
    let pcs = zk_capsule_pcs_verify(
        &proof.source_commitment,
        &proof.mid_commitment,
        &proof.tail,
        gamma,
        beta_source,
        beta_mid,
        &queries,
        &proof.opening,
    )
    .map_err(err)?;
    certify_zk_auth_joint_hiding_rank(
        pcs.source_hiding_rank,
        &owner.terminal_point,
        owner.lambda,
        gamma,
    )
    .map_err(err)?;
    certify_zk_auth_conditioned_companion_hyperplane(
        &owner.post_claim_relation.weights,
        owner.bank_claim(),
        proof.sigma,
        gamma,
    )
    .map_err(err)?;

    Ok(PolicyAuthorizationVerified {
        owner,
        gamma,
        queries: queries.to_vec(),
    })
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

fn run_full_pcs_case(case: &PolicyCase) -> Value {
    const SAMPLES: usize = 3;
    let statement = case.statement();
    let mut prove_ms = Vec::with_capacity(SAMPLES);
    let mut decode_ms = Vec::with_capacity(SAMPLES);
    let mut verify_ms = Vec::with_capacity(SAMPLES);
    let mut wire_bytes = Vec::with_capacity(SAMPLES);
    let mut modeled_bytes = Vec::with_capacity(SAMPLES);
    let mut distinct_queries = Vec::with_capacity(SAMPLES);
    let mut retained: Option<(ZkAuthorizationProof, Vec<u8>)> = None;

    for _ in 0..SAMPLES {
        let started = Instant::now();
        let proof = prove_policy_authorization(case).unwrap();
        prove_ms.push(started.elapsed().as_secs_f64() * 1_000.0);

        let wire = proof.to_bytes().unwrap();
        assert_eq!(wire.len(), proof.serialized_byte_len());
        wire_bytes.push(wire.len());
        modeled_bytes.push(proof.modeled_byte_len());

        let started = Instant::now();
        let decoded = ZkAuthorizationProof::from_bytes(&wire).unwrap();
        decode_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
        let started = Instant::now();
        let verified = verify_policy_authorization(statement, &decoded).unwrap();
        verify_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
        distinct_queries.push(
            verified
                .queries
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
        );
        assert_ne!(verified.owner.lambda, Block256::ZERO);
        assert_ne!(verified.gamma, Block256::ZERO);
        if retained.is_none() {
            retained = Some((proof, wire));
        }
    }

    let (proof, wire) = retained.unwrap();
    let mut negative_checks = 0;
    let mut wrong = statement;
    wrong.program[0][0] += Block128::ONE;
    assert!(verify_policy_authorization(wrong, &proof).is_err());
    negative_checks += 1;
    let mut wrong = statement;
    wrong.program[0][1] += Block128::ONE;
    assert!(verify_policy_authorization(wrong, &proof).is_err());
    negative_checks += 1;
    let mut wrong = statement;
    wrong.current += Block128::ONE;
    assert!(verify_policy_authorization(wrong, &proof).is_err());
    negative_checks += 1;
    let mut wrong = statement;
    wrong.next += Block128::ONE;
    assert!(verify_policy_authorization(wrong, &proof).is_err());
    negative_checks += 1;
    let mut wrong = statement;
    wrong.context += Block128::ONE;
    assert!(verify_policy_authorization(wrong, &proof).is_err());
    negative_checks += 1;
    let mut wrong = statement;
    wrong.call_nonce += Block128::ONE;
    assert!(verify_policy_authorization(wrong, &proof).is_err());
    negative_checks += 1;
    let mut wrong = statement;
    wrong.owner_address[0] += Block128::ONE;
    assert!(verify_policy_authorization(wrong, &proof).is_err());
    negative_checks += 1;
    let mut tampered = wire.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    match ZkAuthorizationProof::from_bytes(&tampered) {
        Ok(decoded) => assert!(verify_policy_authorization(statement, &decoded).is_err()),
        Err(_) => {}
    }
    negative_checks += 1;

    let mut prove_for_median = prove_ms.clone();
    let mut decode_for_median = decode_ms.clone();
    let mut verify_for_median = verify_ms.clone();
    json!({
        "name": case.name,
        "samples": SAMPLES,
        "program_cells_hex": statement.program.map(|instruction| instruction.map(|value| format!("{:032x}", value.0))),
        "current_state_hex": format!("{:032x}", statement.current.0),
        "next_state_hex": format!("{:032x}", statement.next.0),
        "context_hex": format!("{:032x}", statement.context.0),
        "call_nonce_hex": format!("{:032x}", statement.call_nonce.0),
        "owner_address_hex": statement.owner_address.map(|value| format!("{:032x}", value.0)),
        "statement_transcript_lanes": statement.transcript_fields().len(),
        "owner_proof_bytes": proof.owner.byte_len(),
        "prove_ms": prove_ms,
        "prove_median_ms": median(&mut prove_for_median),
        "decode_ms": decode_ms,
        "decode_median_ms": median(&mut decode_for_median),
        "verify_ms": verify_ms,
        "verify_median_ms": median(&mut verify_for_median),
        "wire_bytes": wire_bytes,
        "wire_bytes_min": wire_bytes.iter().min().unwrap(),
        "wire_bytes_max": wire_bytes.iter().max().unwrap(),
        "modeled_bytes": modeled_bytes,
        "distinct_queries": distinct_queries,
        "wire_roundtrip_verified": true,
        "joint_hiding_certificate_recomputed": true,
        "conditioned_companion_certificate_recomputed": true,
        "negative_checks_passed": negative_checks,
    })
}

fn run_case(case: &PolicyCase) -> Value {
    let started = Instant::now();
    let bank = build_bank(case);
    let bank_view = ZkAuthCapsuleBankView::checked(&bank).unwrap();
    let tables = PolicyTables::build(bank_view);
    assert!(tables.validate_boolean_relation());
    let build_ms = started.elapsed().as_secs_f64() * 1_000.0;

    let input_point: [Block256; ZK_AUTH_CAPSULE_BANK_VARS] =
        std::array::from_fn(|index| challenge(index, 0x5050_5248_4F01));
    let lambda = challenge(31, 0x4C41_4D42_4441);
    let round_challenges: [Block256; ZK_AUTH_CAPSULE_BANK_VARS] =
        std::array::from_fn(|index| challenge(index + 64, 0x524F_554E_4401));
    let eta = challenge(127, 0x4554_4101);
    let mask = bank_view.libra_mask_view().unwrap();
    let mask_at_input = mask.evaluate_mle(&input_point);
    let mut main_claim = Block256::ZERO;
    let mut proof_rounds = Vec::with_capacity(ZK_AUTH_CAPSULE_BANK_VARS);
    let mut observed_main_degree = 0;

    let started = Instant::now();
    for round_index in 0..ZK_AUTH_CAPSULE_BANK_VARS {
        let variable = ZK_AUTH_CAPSULE_BANK_VARS - 1 - round_index;
        let main = main_round(&tables, &input_point, &round_challenges[..round_index]);
        observed_main_degree = observed_main_degree.max(max_nonzero_degree(&main));
        assert_eq!(
            mlecheck_endpoint_claim(&main.coeffs, input_point[variable]),
            main_claim
        );
        let mask_round = mask
            .round_coefficients(&input_point, &round_challenges[..round_index])
            .unwrap();
        let combined = combine_main_and_mask_round(&main, &mask_round, lambda).unwrap();
        proof_rounds.push(ZkMleCheckRoundProof::truncate(&combined).unwrap());
        main_claim = main.evaluate(round_challenges[round_index]);
    }
    let terminal_point =
        std::array::from_fn(|variable| round_challenges[ZK_AUTH_CAPSULE_BANK_VARS - 1 - variable]);
    certify_terminal_blinding_rank(&terminal_point).unwrap();
    let operands = terminal_operands(&tables, &terminal_point);
    let terminal = terminal_main(&tables, &terminal_point, operands);
    assert_eq!(main_claim, terminal);
    let mask_at_terminal = mask.evaluate_final(&terminal_point);
    let relation = build_policy_post_claim_relation(
        &input_point,
        &terminal_point,
        operands,
        mask_at_input,
        mask_at_terminal,
        case.boundary(),
        eta,
    );
    assert!(relation.verify(bank_view));
    assert_eq!(
        inner_product(bank_view.cells(), &relation.weights),
        relation.expected_inner_product
    );

    let mut verifier =
        ZkMleCheckVerifierState::new(input_point, Block256::ZERO, mask_at_input, lambda);
    for (round, challenge) in proof_rounds.iter().zip(round_challenges) {
        verifier.transition(round, challenge).unwrap();
    }
    verifier.finish_checked(mask_at_terminal, terminal).unwrap();
    let carrier_ms = started.elapsed().as_secs_f64() * 1_000.0;

    let mut negative_checks = 0;
    let mut invalid_opcode_bank = bank.clone();
    invalid_opcode_bank[state_index(POLICY_BASE_ROW, 2)] = Block128::from(4u128);
    let invalid_opcode_view = ZkAuthCapsuleBankView::checked(&invalid_opcode_bank).unwrap();
    let invalid_opcode_tables = PolicyTables::build(invalid_opcode_view);
    assert!(!invalid_opcode_tables.validate_boolean_relation());
    negative_checks += 1;

    let mut corrupted_bank = bank.clone();
    corrupted_bank[state_index(4, 0)] += Block128::ONE;
    let corrupted_view = ZkAuthCapsuleBankView::checked(&corrupted_bank).unwrap();
    let corrupted_tables = PolicyTables::build(corrupted_view);
    assert!(!corrupted_tables.validate_boolean_relation());
    negative_checks += 1;

    let mut wrong_boundary = case.boundary();
    wrong_boundary.next += Block128::ONE;
    let wrong_relation = build_policy_post_claim_relation(
        &input_point,
        &terminal_point,
        operands,
        mask_at_input,
        mask_at_terminal,
        wrong_boundary,
        eta,
    );
    assert!(!wrong_relation.verify(bank_view));
    negative_checks += 1;

    let mut wrong_boundary = case.boundary();
    wrong_boundary.program[0][1] += Block128::ONE;
    let wrong_relation = build_policy_post_claim_relation(
        &input_point,
        &terminal_point,
        operands,
        mask_at_input,
        mask_at_terminal,
        wrong_boundary,
        eta,
    );
    assert!(!wrong_relation.verify(bank_view));
    negative_checks += 1;

    let digest = case.statement_digest();
    let wrong_digest = policy_statement_digest(
        case.program_cells(),
        case.current,
        case.expected_next() + Block128::ONE,
        case.context,
        case.call_nonce,
    );
    assert_ne!(digest, wrong_digest);
    negative_checks += 1;

    assert_eq!(proof_rounds.len(), ZK_AUTH_CAPSULE_BANK_VARS);
    assert!(proof_rounds
        .iter()
        .all(|round| round.coeffs_without_constant.len() == ZK_MLECHECK_MASK_DEGREE));
    assert_eq!(relation.weights.len(), ZK_AUTH_CAPSULE_BANK_LEN);

    json!({
        "name": case.name,
        "program_cells_hex": case.program_cells().map(|instruction| instruction.map(|value| format!("{:032x}", value.0))),
        "program_bytes": PROGRAM_BYTES,
        "program_steps": PROGRAM_STEPS,
        "current_state_hex": format!("{:032x}", case.current.0),
        "next_state_hex": format!("{:032x}", case.expected_next().0),
        "context_hex": format!("{:032x}", case.context.0),
        "statement_digest_hex": digest.map(|value| format!("{:032x}", value.0)),
        "bank_cells": bank.len(),
        "state_cells": ZK_AUTH_CAPSULE_STATE_LEN,
        "owner_trace_rows_used": ZK_AUTH_CAPSULE_ACTIVE_ROUNDS + 1,
        "policy_trace_start_row": POLICY_BASE_ROW,
        "policy_trace_rows_used": PROGRAM_STEPS + 1,
        "total_nonoverlapping_state_rows_used": ZK_AUTH_CAPSULE_ACTIVE_ROUNDS + 1 + PROGRAM_STEPS + 1,
        "stored_trace_rows": STORED_ROWS,
        "mle_variables": ZK_AUTH_CAPSULE_BANK_VARS,
        "main_relation_max_degree": observed_main_degree,
        "masked_round_degree": ZK_MLECHECK_MASK_DEGREE,
        "rounds": proof_rounds.len(),
        "serialized_coefficients_per_round": ZK_MLECHECK_MASK_DEGREE,
        "terminal_dynamic_operand_claims": operands.ordered().len(),
        "public_boundary_claims": 4 + PROGRAM_CELLS + 3,
        "algebraic_owner_payload_fields_unchanged": 1 + ZK_AUTH_CAPSULE_BANK_VARS * ZK_MLECHECK_MASK_DEGREE + 1 + 5,
        "shape_build_ms": build_ms,
        "explicit_masked_carrier_ms": carrier_ms,
        "negative_checks_passed": negative_checks,
    })
}

fn cases() -> [PolicyCase; 2] {
    let nop = Instruction {
        opcode: Opcode::Nop,
        immediate: 0,
    };
    let mut arithmetic = [nop; PROGRAM_STEPS];
    arithmetic[0] = Instruction {
        opcode: Opcode::AddImmediate,
        immediate: 5,
    };
    arithmetic[1] = Instruction {
        opcode: Opcode::MultiplyImmediate,
        immediate: 3,
    };

    let mut context_select = [nop; PROGRAM_STEPS];
    context_select[0] = Instruction {
        opcode: Opcode::SelectContextImmediate,
        immediate: 42,
    };
    context_select[1] = Instruction {
        opcode: Opcode::MultiplyImmediate,
        immediate: 7,
    };

    [
        PolicyCase {
            name: "arithmetic_state_transition",
            owner_secret: [Block128::from(0xA11CEu128), Block128::from(0xB0Bu128)],
            program: arithmetic,
            current: Block128::from(9u128),
            context: Block128::ZERO,
            call_nonce: Block128::from(17u128),
        },
        PolicyCase {
            name: "context_selected_state_transition",
            owner_secret: [Block128::from(0xC1F3Eu128), Block128::from(0xD371Au128)],
            program: context_select,
            current: Block128::from(11u128),
            context: Block128::ONE,
            call_nonce: Block128::from(18u128),
        },
    ]
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
    assert!(args.len() <= 2, "usage: policy_capsule [NEW_JSON_FILE]");
    assert_eq!(ZK_AUTH_CAPSULE_BANK_LEN, 2_048);
    assert_eq!(ZK_AUTH_CAPSULE_STATE_LEN, 512);
    assert_eq!(ZK_AUTH_CAPSULE_LIBRA_MASK_OFFSET, 512);
    assert_eq!(ZK_AUTH_CAPSULE_LIBRA_MASK_LEN, 256);
    assert_eq!(ZK_AUTH_CAPSULE_REMAINING_PADDING_OFFSET, 768);
    assert_eq!(ZK_AUTH_CAPSULE_PCS_COINS_OFFSET, 1_024);

    let policy_cases = cases();
    let algebraic_cases = policy_cases.clone().map(|case| run_case(&case));
    let full_pcs_cases = policy_cases.map(|case| run_full_pcs_case(&case));
    let result = json!({
        "schema": 3,
        "unix_time": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "source_revision": String::from_utf8(
            std::process::Command::new("git").args(["rev-parse", "HEAD"]).output().unwrap().stdout
        ).unwrap().trim(),
        "kind": "composite_private_owner_plus_universal_in_trace_program_complete_native_pcs_and_wire_not_historystep",
        "same_verifier_algorithm_for_distinct_non_whitelisted_programs": true,
        "same_existing_bank_geometry": true,
        "algebraic_cases": algebraic_cases,
        "full_pcs_cases": full_pcs_cases,
        "established": [
            "the current 66-round private owner permutation and a policy trace coexist in nonoverlapping rows of one committed State table",
            "one unified degree-ten Boolean relation enforces owner knowledge and policy execution",
            "two distinct exact 256-byte in-trace programs use one fixed transition relation without verifier-selected coefficient tables",
            "existing 2^11 bank and 512-cell State-table geometry",
            "eleven masked MLE-check rounds with degree-ten serialized shape",
            "the composite relation retains the same five terminal dynamic operand claims",
            "all sixteen program cells plus current State, next State, and context are sparse boundary claims",
            "program, State, context and call nonce are bound into the Fiat-Shamir statement digest",
            "fresh authorization PCS commitment and complete Phase A/Phase B proof",
            "canonical allocation-bounded wire encode/decode roundtrip",
            "complete native verification after wire decoding",
            "joint hiding and conditioned companion certificates recompute",
            "proof payload keeps the current authorization wire geometry and byte ceiling"
        ],
        "not_established": [
            "recursive HistoryStep acceptance",
            "binding the research policy statement to the production Tx8x2 body and BlockSpine object roots",
            "branches, memory, bounded integers, multiple objects and read-only references",
            "privacy of contract State",
            "soundness accounting for a changed relation"
        ]
    });
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    if let Some(path) = args.get(1) {
        save_new(path, &result);
    }
}
