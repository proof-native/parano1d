// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Conditional depth-aware resource accounting. This module evaluates the
//! declared batch gate-depth prices and scalar gate charge. Construction
//! inventories and universal circuit lower bounds are audited separately.

use num_bigint::BigUint;
use num_rational::Ratio;
use num_traits::{One, Zero};

use crate::{
    exact::{ExactProbability, decimal_ratio_prefix, descriptive_log2_ratio},
    local::{
        HistorySelection, WalletLocalCertificate, select_resource_history, wallet_local_certificate,
    },
    parameters::ProductionParameters,
    qrom::history_query_squeeze_permutations,
};

pub const NIST_CATEGORY_ONE_GD_BITS: u32 = 170;
pub const NIST_CATEGORY_ONE_MAX_DEPTH_BITS: [u32; 3] = [40, 64, 96];
pub const PARALLEL_TRANSITION_CONSTANT: u32 = 10;
pub const HALF_SUCCESS_RESOURCE_DENOMINATOR: u32 = 20;

/// Historical polynomial-multiplier subtotal, excluding field reduction.
/// Retained solely to reproduce the declared resource premise, not as a
/// complete construction or a proved minimum. See `response_audit`.
pub const POLYNOMIAL_MULTIPLIER_LOGICAL_GATES: u64 = 49_023;
pub const POLYNOMIAL_MULTIPLIER_LOGICAL_DEPTH: u64 = 43;

/// Rational upper bound used instead of a floating-point value of Euler's e.
pub const E_UPPER_NUMERATOR: u64 = 2_719;
pub const E_UPPER_DENOMINATOR: u64 = 1_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoherentResponseCost {
    /// Reference factor. For the scalar type this is also the declared
    /// minimum gate charge used to bound the total number of responses.
    pub logical_gates: BigUint,
    /// Reference factor in the gate-depth price, NOT a minimum depth that
    /// every implementation must attain separately.
    pub logical_depth: BigUint,
}

impl CoherentResponseCost {
    pub fn gate_depth_product(&self) -> BigUint {
        &self.logical_gates * &self.logical_depth
    }

    pub fn sequential_permutations(&self, permutations: u32) -> Self {
        Self {
            logical_gates: &self.logical_gates * permutations,
            logical_depth: &self.logical_depth * permutations,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceEvent {
    pub id: &'static str,
    pub bad_density: ExactProbability,
    pub response_cost: CoherentResponseCost,
    pub bad_density_per_gate_depth: ExactProbability,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedFiniteBreakdown {
    pub database_query_cap: BigUint,
    pub extraction_instability: ExactProbability,
    pub transcript_collision_instability: ExactProbability,
    pub total: ExactProbability,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactWorkFloor(Ratio<BigUint>);

impl ExactWorkFloor {
    pub fn from_maximum_ratio(maximum: &ExactProbability) -> Self {
        assert!(!maximum.numerator().is_zero());
        Self(Ratio::new(
            maximum.denominator().clone(),
            maximum.numerator() * HALF_SUCCESS_RESOURCE_DENOMINATOR,
        ))
    }

    pub fn numerator(&self) -> &BigUint {
        self.0.numer()
    }

    pub fn denominator(&self) -> &BigUint {
        self.0.denom()
    }

    pub fn exact_fraction(&self) -> String {
        format!("{}/{}", self.numerator(), self.denominator())
    }

    pub fn decimal_prefix(&self, digits: usize) -> String {
        decimal_ratio_prefix(self.numerator(), self.denominator(), digits)
    }

    pub fn descriptive_bits(&self) -> f64 {
        descriptive_log2_ratio(self.numerator(), self.denominator())
    }

    pub fn exceeds_power_of_two(&self, bits: u32) -> bool {
        self.numerator() > &(self.denominator() << bits)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryOneCertificate {
    pub poseidon_response_cost: CoherentResponseCost,
    pub wallet: WalletLocalCertificate,
    pub history: HistorySelection,
    pub events: Vec<ResourceEvent>,
    pub limiting_event: &'static str,
    pub maximum_bad_density_per_gate_depth: ExactProbability,
    pub dominant_half_success_gate_depth_floor: ExactWorkFloor,
    pub evaluated_max_depth_bits: [u32; 3],
    pub worst_case_max_depth_bits: u32,
    pub category_one_main_term: ExactProbability,
    pub typed_finite: TypedFiniteBreakdown,
    pub global_collision_term: ExactProbability,
    pub ideal_envelope: ExactProbability,
    pub fixed_poseidon2b_delta_headroom: ExactProbability,
}

pub(crate) fn resource_event(
    id: &'static str,
    bad_density: ExactProbability,
    response_cost: CoherentResponseCost,
) -> ResourceEvent {
    let bad_density_per_gate_depth = bad_density.divide_integer(response_cost.gate_depth_product());
    ResourceEvent {
        id,
        bad_density,
        response_cost,
        bad_density_per_gate_depth,
    }
}

/// Declared lower-cost premise, not an audited complete response schedule.
/// The independent scalar target-touch lower bound is only gates>=128,
/// depth>=1 and does not establish these values or vector-query charging.
pub fn poseidon2b_response_cost(parameters: &ProductionParameters) -> CoherentResponseCost {
    let full_sboxes = parameters.poseidon_full_rounds * parameters.poseidon_state_width;
    let partial_sboxes = parameters.poseidon_partial_rounds;
    let sboxes = full_sboxes + partial_sboxes;
    // x^7 has two sequential multiplications when squaring is linear. A
    // coherent response computes and uncomputes the nonlinear schedule.
    let multiplications_per_sbox = 4usize;
    let multiplications_per_round_depth = 4usize;
    let rounds = parameters.poseidon_full_rounds + parameters.poseidon_partial_rounds;
    CoherentResponseCost {
        logical_gates: BigUint::from(sboxes * multiplications_per_sbox)
            * POLYNOMIAL_MULTIPLIER_LOGICAL_GATES,
        logical_depth: BigUint::from(rounds * multiplications_per_round_depth)
            * POLYNOMIAL_MULTIPLIER_LOGICAL_DEPTH,
    }
}

#[cfg(test)]
fn legacy_global_collision_upper(
    parameters: &ProductionParameters,
    scalar_cost: &CoherentResponseCost,
    max_depth_bits: u32,
) -> ExactProbability {
    assert!(max_depth_bits < NIST_CATEGORY_ONE_GD_BITS);
    let gate_cap = BigUint::one() << (NIST_CATEGORY_ONE_GD_BITS - max_depth_bits);
    let depth_cap = BigUint::one() << max_depth_bits;
    let total_queries = &gate_cap / &scalar_cost.logical_gates;
    let query_rounds = &depth_cap / &scalar_cost.logical_depth;
    let square_root_argument = BigUint::from(20u32) * &query_rounds;
    let square_root_ceiling = ceil_square_root(&square_root_argument);
    let e_numerator = BigUint::from(E_UPPER_NUMERATOR);
    let e_denominator = BigUint::from(E_UPPER_DENOMINATOR);

    // Directed-rational evaluation of CFHL Theorem 5.29 after the
    // non-uniform-width Cauchy bound. With N total coherent queries and R
    // query rounds, the amplitude is at most
    //
    //   2 e N sqrt(10 R / 2^digest_bits) + sqrt(2 / 2^digest_bits).
    //
    // `2719/1000 > e` and `ceil(sqrt(20 R))` make every term an exact upper
    // bound while retaining the positive cross term.
    let numerator =
        BigUint::from(40u32) * e_numerator.pow(2) * total_queries.pow(2) * &query_rounds
            + BigUint::from(4u32)
                * &e_numerator
                * &e_denominator
                * &total_queries
                * square_root_ceiling
            + BigUint::from(2u32) * e_denominator.pow(2);
    let denominator = e_denominator.pow(2) * (BigUint::one() << parameters.digest_bits);
    ExactProbability::new(numerator, denominator)
}

fn ceil_square_root(value: &BigUint) -> BigUint {
    if value.is_zero() {
        return BigUint::zero();
    }
    let mut estimate = BigUint::one() << value.bits().div_ceil(2);
    loop {
        let next = (&estimate + value / &estimate) >> 1usize;
        if next >= estimate {
            return if estimate.pow(2) == *value {
                estimate
            } else {
                estimate + 1u32
            };
        }
        estimate = next;
    }
}

/// Collision bound under the batch gate-depth charging premise.
///
/// For each query batch s of k_s scalar responses, assume actual gate/depth
/// charges A_s, delta_s satisfy A_s * delta_s >= k_s * c0, with sum A_s <= G
/// and sum delta_s <= D. Also assume the total count N <= floor(G / g_min).
/// This DOES NOT assume every implementation has depth >= a reference d0.
///
/// Weighted Cauchy gives sum sqrt(k_s) <= sqrt(GD/c0). The CFHL collision
/// transition amplitude is at most 2e sqrt(10 N GD/(c0 M)); add sqrt(2/M)
/// for the output/database comparison. Any required verification queries
/// must already be counted in the budget. Circuit hardness and the all-root
/// specialization are separate from this algebraic resource lemma.
pub fn collision_upper_from_batch_charges(
    parameters: &ProductionParameters,
    scalar_reference: &CoherentResponseCost,
    max_depth_bits: u32,
) -> ExactProbability {
    assert!(max_depth_bits < NIST_CATEGORY_ONE_GD_BITS);
    let gate_cap = BigUint::one() << (NIST_CATEGORY_ONE_GD_BITS - max_depth_bits);
    let total_queries = gate_cap / &scalar_reference.logical_gates;
    let gate_depth_cap = BigUint::one() << NIST_CATEGORY_ONE_GD_BITS;
    let charge = scalar_reference.gate_depth_product();
    let weighted_queries = total_queries * gate_depth_cap;
    let root_numerator = BigUint::from(20u32) * &weighted_queries;
    let root_ceiling = ceil_square_root(&((&root_numerator + &charge - 1u32) / &charge));
    let e_num = BigUint::from(E_UPPER_NUMERATOR);
    let e_den = BigUint::from(E_UPPER_DENOMINATOR);
    let numerator = BigUint::from(40u32) * e_num.pow(2) * weighted_queries
        + BigUint::from(4u32) * &e_num * &e_den * &root_ceiling * &charge
        + BigUint::from(2u32) * e_den.pow(2) * &charge;
    let denominator = e_den.pow(2) * charge * (BigUint::one() << parameters.digest_bits);
    ExactProbability::new(numerator, denominator)
}

fn typed_finite_upper(
    parameters: &ProductionParameters,
    scalar_cost: &CoherentResponseCost,
    max_depth_bits: u32,
) -> TypedFiniteBreakdown {
    assert!(max_depth_bits < NIST_CATEGORY_ONE_GD_BITS);
    let gate_cap = BigUint::one() << (NIST_CATEGORY_ONE_GD_BITS - max_depth_bits);
    let database_query_cap = &gate_cap / &scalar_cost.logical_gates;
    let extraction_bad_density = ExactProbability::dyadic(
        &database_query_cap + BigUint::one(),
        parameters.challenge_min_entropy_bits,
    );
    let collision_bad_density = ExactProbability::dyadic(
        database_query_cap.clone(),
        parameters.challenge_min_entropy_bits,
    );
    // The typed all-root transition contributes extraction instability
    // `(T+1)/|Y|` and transcript-collision instability `T/|Y|`. CFHL's
    // parallel transition bound applies the same `10*G*D/(g*d)` resource
    // factor to their sum. A scalar response is simultaneously the cheapest
    // response and the smallest production response support, so it maximizes
    // this finite term.
    let resource_factor =
        BigUint::from(PARALLEL_TRANSITION_CONSTANT) * (BigUint::one() << NIST_CATEGORY_ONE_GD_BITS);
    let lift = |density: &ExactProbability| {
        density
            .divide_integer(scalar_cost.gate_depth_product())
            .scale_integer(resource_factor.clone())
    };
    let extraction_instability = lift(&extraction_bad_density);
    let transcript_collision_instability = lift(&collision_bad_density);
    let total = extraction_instability.add(&transcript_collision_instability);
    TypedFiniteBreakdown {
        database_query_cap,
        extraction_instability,
        transcript_collision_instability,
        total,
    }
}

pub fn certificate(parameters: &ProductionParameters) -> CategoryOneCertificate {
    let poseidon_response_cost = poseidon2b_response_cost(parameters);
    let wallet = wallet_local_certificate(parameters);
    let history_query_permutations = history_query_squeeze_permutations(parameters);
    let history = select_resource_history(parameters, history_query_permutations);
    let wallet_query_permutations = u32::try_from(
        parameters
            .wallet_query_seed_lanes
            .div_ceil(parameters.poseidon_rate_lanes),
    )
    .expect("wallet squeeze count fits u32");

    let scalar = poseidon_response_cost.clone();
    let wallet_query = scalar.sequential_permutations(wallet_query_permutations);
    let history_query = scalar.sequential_permutations(history_query_permutations);
    let events = vec![
        resource_event("wallet.query", wallet.query_escape.clone(), wallet_query),
        resource_event(
            "wallet.field",
            wallet.field_exception.clone(),
            scalar.clone(),
        ),
        resource_event(
            "history.query",
            history.certificate.query_escape.clone(),
            history_query,
        ),
        resource_event(
            "history.b25.proximity",
            history.certificate.classes[0].proximity_exception.clone(),
            scalar.clone(),
        ),
        resource_event(
            "history.b255.proximity",
            history.certificate.classes[1].proximity_exception.clone(),
            scalar.clone(),
        ),
        resource_event(
            "history.candidate-switching",
            history.certificate.candidate_switching_exception.clone(),
            scalar.clone(),
        ),
        resource_event(
            "history.joint-sidecar",
            history.certificate.joint_sidecar_exception.clone(),
            scalar.clone(),
        ),
    ];
    compose_events(parameters, wallet, history, events)
}

/// Apply the existing all-root resource calculation to an explicitly supplied
/// inventory. Callers must establish each event's local RBR bound separately.
pub(crate) fn compose_events(
    parameters: &ProductionParameters,
    wallet: WalletLocalCertificate,
    history: HistorySelection,
    mut events: Vec<ResourceEvent>,
) -> CategoryOneCertificate {
    let poseidon_response_cost = poseidon2b_response_cost(parameters);
    assert!(!events.is_empty(), "resource inventory must not be empty");
    events.sort_by(|left, right| {
        right
            .bad_density_per_gate_depth
            .cmp(&left.bad_density_per_gate_depth)
    });
    let limiting_event = events[0].id;
    let maximum_bad_density_per_gate_depth = events[0].bad_density_per_gate_depth.clone();
    let dominant_half_success_gate_depth_floor =
        ExactWorkFloor::from_maximum_ratio(&maximum_bad_density_per_gate_depth);

    let category_one_main_term = maximum_bad_density_per_gate_depth.scale_integer(
        BigUint::from(PARALLEL_TRANSITION_CONSTANT) * (BigUint::one() << NIST_CATEGORY_ONE_GD_BITS),
    );

    let (worst_case_max_depth_bits, typed_finite, global_collision_term, ideal_envelope) =
        NIST_CATEGORY_ONE_MAX_DEPTH_BITS
            .into_iter()
            .map(|max_depth_bits| {
                let typed_finite =
                    typed_finite_upper(parameters, &poseidon_response_cost, max_depth_bits);
                let global_collision_term = collision_upper_from_batch_charges(
                    parameters,
                    &poseidon_response_cost,
                    max_depth_bits,
                );
                let ideal_envelope = category_one_main_term
                    .add(&typed_finite.total)
                    .add(&global_collision_term);
                (
                    max_depth_bits,
                    typed_finite,
                    global_collision_term,
                    ideal_envelope,
                )
            })
            .max_by(|left, right| left.3.cmp(&right.3))
            .expect("NIST Category 1 has reference MAXDEPTH points");
    let fixed_poseidon2b_delta_headroom = ExactProbability::new(1u32, 2u32)
        .checked_sub(&ideal_envelope)
        .expect("Category 1 ideal envelope remains below half success");

    CategoryOneCertificate {
        poseidon_response_cost,
        wallet,
        history,
        events,
        limiting_event,
        maximum_bad_density_per_gate_depth,
        dominant_half_success_gate_depth_floor,
        evaluated_max_depth_bits: NIST_CATEGORY_ONE_MAX_DEPTH_BITS,
        worst_case_max_depth_bits,
        category_one_main_term,
        typed_finite,
        global_collision_term,
        ideal_envelope,
        fixed_poseidon2b_delta_headroom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_response_premise_reproduces_the_historical_subtotal() {
        let parameters = ProductionParameters::load().unwrap();
        let cost = poseidon2b_response_cost(&parameters);
        assert_eq!(cost.logical_gates, BigUint::from(17_648_280u64));
        assert_eq!(cost.logical_depth, BigUint::from(11_352u64));
        assert_eq!(cost.gate_depth_product(), BigUint::from(200_343_274_560u64));
    }

    #[test]
    fn category_one_resource_floor_exceeds_two_to_170() {
        let parameters = ProductionParameters::load().unwrap();
        let result = certificate(&parameters);
        assert_eq!(result.limiting_event, "history.query");
        assert_eq!(result.evaluated_max_depth_bits, [40, 64, 96]);
        assert_eq!(result.worst_case_max_depth_bits, 40);
        assert!(
            result
                .dominant_half_success_gate_depth_floor
                .exceeds_power_of_two(170)
        );
        assert!(
            result
                .dominant_half_success_gate_depth_floor
                .descriptive_bits()
                > 173.39
        );
        assert!(result.ideal_envelope < ExactProbability::new(1u32, 2u32));
        assert!(result.fixed_poseidon2b_delta_headroom > ExactProbability::new(45u32, 100u32));
        assert_eq!(
            result.typed_finite.total.decimal_prefix(15),
            "0.000199022715317"
        );
        assert_eq!(
            result.global_collision_term.decimal_prefix(15),
            "0.001471367172458"
        );
    }

    #[test]
    fn finite_envelope_is_largest_at_the_smallest_nist_maxdepth() {
        let parameters = ProductionParameters::load().unwrap();
        let cost = poseidon2b_response_cost(&parameters);
        let envelope = |depth_bits| {
            typed_finite_upper(&parameters, &cost, depth_bits)
                .total
                .add(&collision_upper_from_batch_charges(
                    &parameters,
                    &cost,
                    depth_bits,
                ))
        };
        assert!(envelope(40) > envelope(64));
        assert!(envelope(64) > envelope(96));
    }

    #[test]
    fn directed_integer_square_root_is_an_upper_bound() {
        for value in 0u32..1000 {
            let value = BigUint::from(value);
            let ceiling = ceil_square_root(&value);
            assert!(ceiling.pow(2) >= value);
            if !ceiling.is_zero() {
                assert!((&ceiling - 1u32).pow(2) < value);
            }
        }
    }

    #[test]
    fn batch_charge_lemma_does_not_need_a_universal_response_depth_minimum() {
        let parameters = ProductionParameters::load().unwrap();
        let reference = poseidon2b_response_cost(&parameters);
        let historical = certificate(&parameters);
        for depth_bits in NIST_CATEGORY_ONE_MAX_DEPTH_BITS {
            let corrected = collision_upper_from_batch_charges(&parameters, &reference, depth_bits);
            assert!(
                corrected >= legacy_global_collision_upper(&parameters, &reference, depth_bits)
            );
            let envelope = historical
                .category_one_main_term
                .add(&typed_finite_upper(&parameters, &reference, depth_bits).total)
                .add(&corrected);
            assert!(envelope < ExactProbability::new(1u32, 2u32));
            println!(
                "batch-charge MAXDEPTH=2^{depth_bits}: collision <= {}, conditional envelope <= {}",
                corrected.decimal_ceiling(18),
                envelope.decimal_ceiling(18)
            );
        }
    }

    #[test]
    fn removing_reference_depth_floor_rounding_is_exactly_directed() {
        let parameters = ProductionParameters::load().unwrap();
        // With unit prices, no floor was lost, so the squared leading terms
        // coincide; rounding the whole cross term can only tighten it.
        let reference = CoherentResponseCost {
            logical_gates: BigUint::one(),
            logical_depth: BigUint::one(),
        };
        for depth_bits in NIST_CATEGORY_ONE_MAX_DEPTH_BITS {
            // sqrt(20*N*GD) = N*sqrt(20*D) before directed rounding; the new
            // expression rounds the whole cross term and is at least as tight.
            assert!(
                collision_upper_from_batch_charges(&parameters, &reference, depth_bits)
                    <= legacy_global_collision_upper(&parameters, &reference, depth_bits)
            );
        }
    }
}
