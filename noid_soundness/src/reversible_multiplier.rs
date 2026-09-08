// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Complete reversible GF(2^128) multiplication in the production flat basis.
//! Construction counts are upper bounds, separate from the resource premise.

#[derive(Clone, Copy, Debug)]
enum Gate {
    Cnot(usize, usize),
    Toffoli(usize, usize, usize),
}

#[derive(Default)]
struct Circuit {
    gates: Vec<Gate>,
    last_layer: Vec<u64>,
    cnots: u64,
    toffolis: u64,
}

impl Circuit {
    fn wires(&mut self, count: usize) -> Vec<usize> {
        let start = self.last_layer.len();
        self.last_layer.resize(start + count, 0);
        (start..start + count).collect()
    }

    fn push(&mut self, gate: Gate) {
        match gate {
            Gate::Cnot(a, b) => {
                assert_ne!(a, b);
                let layer = self.last_layer[a].max(self.last_layer[b]) + 1;
                self.last_layer[a] = layer;
                self.last_layer[b] = layer;
                self.cnots += 1;
            }
            Gate::Toffoli(a, b, c) => {
                assert!(a != b && b != c && a != c);
                let layer = self.last_layer[a]
                    .max(self.last_layer[b])
                    .max(self.last_layer[c])
                    + 8;
                for wire in [a, b, c] {
                    self.last_layer[wire] = layer;
                }
                self.toffolis += 1;
            }
        }
        self.gates.push(gate);
    }

    fn depth(&self) -> u64 {
        self.last_layer.iter().copied().max().unwrap_or(0)
    }
    fn logical_gates(&self) -> u64 {
        self.cnots + 15 * self.toffolis
    }

    /// Preserve every input wire; generate each output as a balanced parity
    /// of dedicated copies. Columns describe a square binary linear map.
    #[cfg(test)]
    fn linear(&mut self, input: &[usize], columns: &[u128]) -> Vec<usize> {
        let n = input.len();
        assert_eq!(columns.len(), n);
        assert!(n.is_power_of_two() && n <= 128);
        let copies = input
            .iter()
            .map(|&source| {
                let first = self.wires(1)[0];
                self.push(Gate::Cnot(source, first));
                let mut copies = vec![first];
                while copies.len() < n {
                    for source in copies.clone() {
                        if copies.len() == n {
                            break;
                        }
                        let target = self.wires(1)[0];
                        self.push(Gate::Cnot(source, target));
                        copies.push(target);
                    }
                }
                copies
            })
            .collect::<Vec<_>>();
        let outputs = self.wires(n);
        for (bit, &output) in outputs.iter().enumerate() {
            let mut terms = (0..n)
                .filter(|&column| (columns[column] >> bit) & 1 != 0)
                .map(|column| copies[column][bit])
                .collect::<Vec<_>>();
            while terms.len() > 1 {
                let mut next = Vec::new();
                for pair in terms.chunks(2) {
                    if pair.len() == 1 {
                        next.push(pair[0]);
                    } else {
                        self.push(Gate::Cnot(pair[0], pair[1]));
                        next.push(pair[1]);
                    }
                }
                terms = next;
            }
            if let Some(&root) = terms.first() {
                self.push(Gate::Cnot(root, output));
            }
        }
        outputs
    }

    // All controls of Toffoli gates below are linear combinations of input
    // coefficients. The whole resulting polynomial map is therefore bilinear.
    fn polynomial(&mut self, left: &[usize], right: &[usize]) -> Vec<usize> {
        let n = left.len();
        assert_eq!(right.len(), n);
        assert!(n.is_power_of_two());
        if n == 1 {
            let result = self.wires(1)[0];
            self.push(Gate::Toffoli(left[0], right[0], result));
            return vec![result];
        }
        let half = n / 2;
        let sum_left = self.wires(half);
        let sum_right = self.wires(half);
        for i in 0..half {
            self.push(Gate::Cnot(left[i], sum_left[i]));
            self.push(Gate::Cnot(left[i + half], sum_left[i]));
            self.push(Gate::Cnot(right[i], sum_right[i]));
            self.push(Gate::Cnot(right[i + half], sum_right[i]));
        }
        let low = self.polynomial(&left[..half], &right[..half]);
        let high = self.polynomial(&left[half..], &right[half..]);
        let middle = self.polynomial(&sum_left, &sum_right);
        // A + X^(n/2)(A+B+C) + X^n C. Keep the unused wires as garbage.
        // Merge uses exactly 3n-4 CNOTs; preparing the sums used 2n.
        for i in 0..n - 1 {
            self.push(Gate::Cnot(low[i], middle[i]));
        }
        for i in 0..n - 1 {
            self.push(Gate::Cnot(high[i], middle[i]));
        }
        for i in 0..half - 1 {
            self.push(Gate::Cnot(low[i + half], middle[i]));
            self.push(Gate::Cnot(high[i], middle[i + half]));
        }
        low[..half]
            .iter()
            .chain(&middle)
            .chain(&high[half - 1..])
            .copied()
            .collect()
    }

    fn reduce(&mut self, product: &[usize], n: usize, modulus_low: u128) {
        assert_eq!(product.len(), 2 * n - 1);
        for i in (n..2 * n - 1).rev() {
            for bit in 0..n {
                if (modulus_low >> bit) & 1 != 0 {
                    self.push(Gate::Cnot(product[i], product[i - n + bit]));
                }
            }
        }
    }

    #[cfg(test)]
    fn simulate(&self, wires: &mut [u64]) {
        for gate in &self.gates {
            match *gate {
                Gate::Cnot(a, b) => wires[b] ^= wires[a],
                Gate::Toffoli(a, b, c) => wires[c] ^= wires[a] & wires[b],
            }
        }
    }
}

#[allow(dead_code)] // Input wire maps are used by the exhaustive basis tests.
struct Multiplier {
    circuit: Circuit,
    left: Vec<usize>,
    right: Vec<usize>,
    output: Vec<usize>,
    polynomial_gates: u64,
    polynomial_depth: u64,
    forward_gates: u64,
    forward_depth: u64,
}

fn multiplier(n: usize, modulus_low: u128) -> Multiplier {
    let mut circuit = Circuit::default();
    let left = circuit.wires(n);
    let right = circuit.wires(n);
    let output = circuit.wires(n);
    let product = circuit.polynomial(&left, &right);
    let polynomial_gates = circuit.logical_gates();
    let polynomial_depth = circuit.depth();
    circuit.reduce(&product, n, modulus_low);
    let forward_gates = circuit.logical_gates();
    let forward_depth = circuit.depth();
    let compute = circuit.gates.clone();
    for i in 0..n {
        circuit.push(Gate::Cnot(product[i], output[i]));
    }
    for gate in compute.into_iter().rev() {
        circuit.push(gate);
    }
    Multiplier {
        circuit,
        left,
        right,
        output,
        polynomial_gates,
        polynomial_depth,
        forward_gates,
        forward_depth,
    }
}

#[cfg(test)]
fn field_multiply(mut a: u128, mut b: u128, n: usize, modulus_low: u128) -> u128 {
    let mask = if n == 128 {
        u128::MAX
    } else {
        (1u128 << n) - 1
    };
    let mut result = 0;
    for _ in 0..n {
        if b & 1 != 0 {
            result ^= a;
        }
        b >>= 1;
        let carry = a >> (n - 1);
        a = ((a << 1) & mask) ^ if carry != 0 { modulus_low } else { 0 };
    }
    result
}

#[cfg(test)]
fn test_cases(m: &Multiplier, n: usize, modulus: u128, cases: &[(u128, u128, u128)]) {
    for batch in cases.chunks(64) {
        let mut wires = vec![0u64; m.circuit.last_layer.len()];
        for (lane, &(a, b, z)) in batch.iter().enumerate() {
            for bit in 0..n {
                wires[m.left[bit]] |= (((a >> bit) & 1) as u64) << lane;
                wires[m.right[bit]] |= (((b >> bit) & 1) as u64) << lane;
                wires[m.output[bit]] |= (((z >> bit) & 1) as u64) << lane;
            }
        }
        let initial = wires.clone();
        m.circuit.simulate(&mut wires);
        for (lane, &(a, b, z)) in batch.iter().enumerate() {
            let answer = m
                .output
                .iter()
                .enumerate()
                .fold(0u128, |value, (bit, &wire)| {
                    value | (u128::from((wires[wire] >> lane) & 1) << bit)
                });
            assert_eq!(answer, z ^ field_multiply(a, b, n, modulus));
        }
        for wire in 0..wires.len() {
            if !m.output.contains(&wire) {
                assert_eq!(wires[wire], initial[wire], "unclean wire {wire}");
            }
        }
    }
}

#[test]
fn gf16_every_input_pair_and_every_initial_response() {
    let m = multiplier(4, 0x3);
    let cases = (0..16u128)
        .flat_map(|a| (0..16u128).flat_map(move |b| (0..16u128).map(move |z| (a, b, z))))
        .collect::<Vec<_>>();
    test_cases(&m, 4, 0x3, &cases);
}

#[test]
fn production_field_every_bilinear_basis_pair_and_clean_response() {
    let m = multiplier(128, 0x87);
    let cases = (0..128)
        .flat_map(|i| (0..128).map(move |j| (1u128 << i, 1u128 << j, u128::MAX.rotate_left(i + j))))
        .collect::<Vec<_>>();
    test_cases(&m, 128, 0x87, &cases);
    // Bilinearity plus equality on all 128^2 basis pairs proves correctness
    // on every input pair. Inverting the complete circuit cleans every wire.
    test_cases(
        &m,
        128,
        0x87,
        &[
            (u128::MAX, u128::MAX, 0),
            (
                0x31415926535897932384626433832795,
                0xabcdef00123456789abcdef001234567,
                0xdeadbeef,
            ),
            (0, u128::MAX, u128::MAX),
        ],
    );
}

#[test]
fn exact_resource_inventory() {
    let m = multiplier(128, 0x87);
    assert_eq!(m.polynomial_gates, 49_023);
    // The paper's 43-layer bound is conservative: the size-two recursion
    // has no third merge layer. This explicitly scheduled circuit uses 42.
    assert_eq!(m.polynomial_depth, 42);
    assert!(m.polynomial_depth <= 43);
    assert_eq!(m.forward_gates, 49_023 + 508);
    assert_eq!(m.forward_depth, 49);
    assert_eq!(m.circuit.logical_gates(), 2 * (49_023 + 508) + 128);
    assert_eq!(m.circuit.depth(), 99);
    assert_eq!(m.circuit.last_layer.len(), 6_689);
    assert!(m.circuit.depth() <= 2 * m.forward_depth + 1);
    println!(
        "polynomial: {} gates, {} depth",
        m.polynomial_gates, m.polynomial_depth
    );
    println!(
        "field product with retained workspace: {} gates, {} depth",
        m.forward_gates, m.forward_depth
    );
    println!(
        "clean field XOR response: {} gates, {} depth, {} wires",
        m.circuit.logical_gates(),
        m.circuit.depth(),
        m.circuit.last_layer.len()
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldMultiplierAudit {
    pub polynomial_gates: u64,
    pub polynomial_depth: u64,
    pub forward_field_gates: u64,
    pub forward_field_depth: u64,
    pub clean_xor_gates: u64,
    pub clean_xor_depth: u64,
    pub total_wires: usize,
    pub forward_additional_wires: usize,
}

pub fn audit() -> FieldMultiplierAudit {
    let m = multiplier(128, 0x87);
    FieldMultiplierAudit {
        polynomial_gates: m.polynomial_gates,
        polynomial_depth: m.polynomial_depth,
        forward_field_gates: m.forward_gates,
        forward_field_depth: m.forward_depth,
        clean_xor_gates: m.circuit.logical_gates(),
        clean_xor_depth: m.circuit.depth(),
        total_wires: m.circuit.last_layer.len(),
        forward_additional_wires: m.circuit.last_layer.len() - 3 * 128,
    }
}

#[test]
fn arbitrary_binary_linear_map_preserves_inputs_and_has_charged_fanout() {
    let n = 8usize;
    for columns in [
        [0u128; 8],
        [255u128; 8],
        [1, 2, 4, 8, 16, 32, 64, 128],
        [0x91, 0x52, 0x75, 0x23, 0xff, 0xaa, 0x16, 0x80],
    ] {
        let mut circuit = Circuit::default();
        let input = circuit.wires(n);
        let output = circuit.linear(&input, &columns);
        assert!(circuit.logical_gates() <= 2 * (n * n) as u64);
        assert!(circuit.depth() <= 2 * u64::from(n.ilog2()) + 2);
        assert!(circuit.last_layer.len() <= n + n * n + n);
        let compute = circuit.gates.clone();
        for value in 0..256u128 {
            let mut wires = vec![0u64; circuit.last_layer.len()];
            for bit in 0..n {
                wires[input[bit]] = ((value >> bit) & 1) as u64;
            }
            let original = wires.clone();
            circuit.simulate(&mut wires);
            let expected = (0..n)
                .filter(|&bit| (value >> bit) & 1 != 0)
                .fold(0, |value, bit| value ^ columns[bit]);
            let actual = output.iter().enumerate().fold(0, |value, (bit, &wire)| {
                value | (u128::from(wires[wire]) << bit)
            });
            assert_eq!(actual, expected);
            for &wire in &input {
                assert_eq!(wires[wire], original[wire]);
            }
            for gate in compute.iter().rev() {
                match *gate {
                    Gate::Cnot(a, b) => wires[b] ^= wires[a],
                    Gate::Toffoli(..) => unreachable!(),
                }
            }
            assert_eq!(wires, original);
        }
    }
}
