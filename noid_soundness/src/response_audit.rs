// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Reversible component inventories and a complete compositional scalar upper
//! bound, kept separate from the declared Category 1 resource prices.
//!
//! The Karatsuba counts exclude modular reduction. The target-touch lower
//! bound applies to fixed-register, exact unitary XOR responses in the
//! CNOT / one-qubit Clifford / T basis. Neither proves the resource premise.

use noid_core::{Block128, TowerField};

use crate::parameters::ProductionParameters;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolynomialMultiplierSubtotal {
    pub structural_cnots: u64,
    pub toffolis: u64,
    pub decomposed_cnots: u64,
    pub one_qubit_cliffords: u64,
    pub t_gates: u64,
    pub logical_gates: u64,
    pub logical_depth: u64,
}

/// Jang et al., Sensors 23(6), 3156, Table 1; reduction is explicitly excluded.
pub fn polynomial_multiplier_subtotal() -> PolynomialMultiplierSubtotal {
    let levels = 7u32;
    let structural_cnots = (0..levels)
        .map(|level| 3u64.pow(level) * (5 * (128 >> level) - 4))
        .sum::<u64>();
    let toffolis = 3u64.pow(levels);
    let decomposed_cnots = structural_cnots + 6 * toffolis;
    let one_qubit_cliffords = 2 * toffolis;
    let t_gates = 7 * toffolis;
    PolynomialMultiplierSubtotal {
        structural_cnots,
        toffolis,
        decomposed_cnots,
        one_qubit_cliffords,
        t_gates,
        logical_gates: decomposed_cnots + one_qubit_cliffords + t_gates,
        logical_depth: 5 * u64::from(levels) + 8,
    }
}

/// One reversible linear component in the GCM polynomial basis. High wires
/// are retained, not erased. Copy the reduced low lane, then reverse this
/// network before reversing the polynomial multiplier to clean its workspace.
/// Basis conversion, multiplication, output copy and cleanup are not counted.
pub fn gcm_reduction_cnot_schedule() -> Vec<(usize, usize)> {
    let mut gates = Vec::with_capacity(508);
    for control in (128..255).rev() {
        for offset in [0, 1, 2, 7] {
            gates.push((control, control - 128 + offset));
        }
    }
    gates
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResponseAudit {
    pub polynomial_subtotal: PolynomialMultiplierSubtotal,
    pub field_multiplier: crate::reversible_multiplier::FieldMultiplierAudit,
    pub reduction_component_cnots: usize,
    pub external_determinant: u128,
    pub internal_determinant: u128,
    /// Exact fixed-register XOR response, unitary CNOT / 1q Clifford / T only.
    pub scalar_target_touch_gate_lower: u64,
    pub scalar_target_touch_depth_lower: u64,
    pub scalar_construction_upper: ScalarConstructionUpper,
}

/// Complete, deliberately conservative clean-unitary construction. Every
/// binary linear map preserves its inputs and charges fanout and parity.
/// This upper bound must never be substituted for a minimum response price.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalarConstructionUpper {
    pub logical_gates: u64,
    pub logical_depth: u64,
    pub gate_depth: u64,
    pub total_wires: u64,
}

fn scalar_construction_upper(
    p: &ProductionParameters,
    m: &crate::reversible_multiplier::FieldMultiplierAudit,
) -> ScalarConstructionUpper {
    let lane = 128u64;
    let state = lane * p.poseidon_state_width as u64;
    let rounds = (p.poseidon_full_rounds + p.poseidon_partial_rounds) as u64;
    let sboxes =
        (p.poseidon_full_rounds * p.poseidon_state_width + p.poseidon_partial_rounds) as u64;
    let linear_gates = |bits: u64| 2 * bits * bits;
    let linear_depth = |bits: u64| 2 * u64::from(bits.ilog2()) + 2;
    let linear_wires = |bits: u64| bits * bits + bits;
    let sbox_gates = 2 * linear_gates(lane) + 2 * m.forward_field_gates;
    let sbox_depth = 2 * linear_depth(lane) + 2 * m.forward_field_depth;
    let sbox_wires = 2 * linear_wires(lane) + 2 * m.forward_additional_wires as u64;
    // Four tower-to-flat inputs and one flat-to-tower scalar output. Bounds
    // are uniform over the actual pinned linear maps and public constants.
    let conversions = p.poseidon_state_width as u64 + 1;
    let forward_gates = sboxes * sbox_gates
        + (rounds + 1) * linear_gates(state)
        + sboxes * lane
        + conversions * linear_gates(lane);
    let forward_depth = 2 * linear_depth(lane)
        + linear_depth(state)
        + rounds * (1 + sbox_depth + linear_depth(state));
    let logical_gates = 2 * forward_gates + lane;
    let logical_depth = 2 * forward_depth + 1;
    ScalarConstructionUpper {
        logical_gates,
        logical_depth,
        gate_depth: logical_gates * logical_depth,
        total_wires: state
            + lane
            + sboxes * sbox_wires
            + (rounds + 1) * linear_wires(state)
            + conversions * linear_wires(lane),
    }
}

pub fn audit(parameters: &ProductionParameters) -> Result<ResponseAudit, String> {
    if parameters.poseidon_state_width != 4 || parameters.poseidon_sbox_exponent != 7 {
        return Err("response audit requires the pinned width-four x^7 permutation".into());
    }
    let mut a = parameters.poseidon_sbox_exponent as u128;
    let mut b = u128::MAX; // 2^128 - 1, the multiplicative group order.
    while b != 0 {
        (a, b) = (b, a % b);
    }
    if a != 1 {
        return Err("the S-box exponent is not invertible".into());
    }
    let external_determinant = determinant(&parameters.poseidon_external_matrix);
    let internal_determinant = determinant(&parameters.poseidon_internal_matrix);
    if external_determinant == 0 || internal_determinant == 0 {
        return Err("response surjectivity requires invertible linear layers".into());
    }
    let field_multiplier = crate::reversible_multiplier::audit();
    let scalar_construction_upper = scalar_construction_upper(parameters, &field_multiplier);
    Ok(ResponseAudit {
        polynomial_subtotal: polynomial_multiplier_subtotal(),
        field_multiplier,
        reduction_component_cnots: gcm_reduction_cnot_schedule().len(),
        external_determinant,
        internal_determinant,
        scalar_target_touch_gate_lower: 128,
        scalar_target_touch_depth_lower: 1,
        scalar_construction_upper,
    })
}

// Leibniz expansion (24 terms) in characteristic two. This is independent of
// Gaussian elimination and does not assume a special form for either matrix.
fn determinant(matrix: &[[u128; 4]; 4]) -> u128 {
    let mut result = Block128::ZERO;
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    if a == b || a == c || a == d || b == c || b == d || c == d {
                        continue;
                    }
                    let product = [a, b, c, d]
                        .into_iter()
                        .enumerate()
                        .fold(Block128::ONE, |product, (row, column)| {
                            product * Block128::from(matrix[row][column])
                        });
                    result += product;
                }
            }
        }
    }
    result.to_u128()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(wires: &mut [bool; 255], gates: impl Iterator<Item = (usize, usize)>) {
        for (control, target) in gates {
            assert_ne!(control, target);
            wires[target] ^= wires[control];
        }
    }

    fn low_lane(wires: &[bool; 255]) -> u128 {
        (0..128).fold(0, |value, bit| value | (u128::from(wires[bit]) << bit))
    }

    #[test]
    fn published_counts_are_reconstructed_without_a_field_reduction() {
        let subtotal = polynomial_multiplier_subtotal();
        assert_eq!(subtotal.structural_cnots, 16_218);
        assert_eq!(subtotal.toffolis, 2_187);
        assert_eq!(subtotal.decomposed_cnots, 29_340);
        assert_eq!(subtotal.one_qubit_cliffords, 4_374);
        assert_eq!(subtotal.t_gates, 15_309);
        assert_eq!(subtotal.logical_gates, 49_023);
        assert_eq!(subtotal.logical_depth, 43);
    }

    #[test]
    fn reduction_matches_every_basis_vector_and_reverses_exactly() {
        let gates = gcm_reduction_cnot_schedule();
        assert_eq!(gates.len(), 508);
        // Multiplication by x modulo x^128+x^7+x^2+x+1 independently gives
        // every basis image. Equality on the basis proves the linear maps equal.
        let mut expected = 1u128;
        for degree in 0..255 {
            let mut wires = [false; 255];
            wires[degree] = true;
            let original = wires;
            apply(&mut wires, gates.iter().copied());
            assert_eq!(low_lane(&wires), expected, "coefficient {degree}");
            apply(&mut wires, gates.iter().rev().copied());
            assert_eq!(wires, original, "reversal at coefficient {degree}");
            let carry = expected >> 127;
            expected = (expected << 1) ^ if carry != 0 { 0x87 } else { 0 };
        }
    }

    #[test]
    fn unreduced_product_is_not_the_field_product() {
        // x * x^127 = x^128: its unreduced low 128 coefficients are zero.
        let mut wires = [false; 255];
        wires[128] = true;
        assert_eq!(low_lane(&wires), 0);
        apply(&mut wires, gcm_reduction_cnot_schedule().into_iter());
        assert_eq!(low_lane(&wires), 0x87);
    }

    #[test]
    fn production_permutation_has_a_surjective_scalar_projection() {
        let result = audit(&ProductionParameters::load().unwrap()).unwrap();
        assert_eq!(result.external_determinant, 0x40);
        assert_eq!(result.internal_determinant, 0x2064);
        assert_eq!(result.scalar_target_touch_gate_lower, 128);
        assert_eq!(result.scalar_target_touch_depth_lower, 1);
    }

    #[test]
    fn complete_scalar_upper_includes_linear_maps_and_preserves_reference_prices() {
        let parameters = ProductionParameters::load().unwrap();
        let result = audit(&parameters).unwrap();
        let upper = &result.scalar_construction_upper;
        assert_eq!(upper.logical_gates, 100_233_080);
        assert_eq!(upper.logical_depth, 20_037);
        assert_eq!(upper.total_wires, 21_788_212);
        assert_eq!(upper.gate_depth, upper.logical_gates * upper.logical_depth);
        let price = crate::resource::poseidon2b_response_cost(&parameters);
        assert_eq!(price.gate_depth_product().to_string(), "200343274560");
        assert!(num_bigint::BigUint::from(upper.gate_depth) > price.gate_depth_product());
        assert_eq!(result.scalar_target_touch_gate_lower, 128);
        // Neither an expensive construction nor a weak lower bound is fed
        // back into the stronger, separately stated resource-price premise.
    }

    #[test]
    fn a_singular_matrix_cannot_inherit_the_lower_bound_certificate() {
        let mut parameters = ProductionParameters::load().unwrap();
        parameters.poseidon_internal_matrix[1] = parameters.poseidon_internal_matrix[0];
        assert!(audit(&parameters).is_err());
    }
}
