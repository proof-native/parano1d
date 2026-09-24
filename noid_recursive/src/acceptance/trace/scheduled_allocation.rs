// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Exact in-circuit stateless development-allocation schedule.

use noid_chain::consensus::development_allocation::{
    development_allocation_with_schedule, DEVELOPMENT_ALLOCATION_END_HEIGHT, TARGET_BLOCKS_PER_DAY,
};
use noid_chain::consensus::forks::{ForkSchedule, V2Activation};
use noid_core::Block128;

use super::exact_state::{StateDepthTrace, MAX_EXACT_STATE_DEPTH, MIN_EXACT_STATE_DEPTH};
use super::{
    alloc_block, const_block, flat_const, integer_add_no_overflow, mul, pin_eq, pin_lt_strict,
    range_check_bits, FieldR1csBuilder, LinExpr, Wire, F128,
};

const HEIGHT_BITS: usize = 64;
// The daily interval changes with the target, while the allocation end height
// stays fixed. Four shifted terms cover each supported divisor without adding
// an integer multiplier; v2 needs one more quotient bit for full u64 heights.

pub struct DevelopmentAllocationTrace {
    /// Canonical range-check bits of the accepted child height. Consumers
    /// such as the v2 timed object policy reuse these exact wires.
    pub height_bits: [Wire; HEIGHT_BITS],
    /// Authenticated v2 activation bit, also used by contract admission in the
    /// local fork relation. It is not a witness-selected protocol version.
    pub v2_active: LinExpr,
    pub active: LinExpr,
    pub payout_due: LinExpr,
    pub share_each: LinExpr,
    pub miner_subsidy: LinExpr,
    pub payout_each: LinExpr,
}

/// Reserve the schedule outputs used by earlier slots, then bind every one
/// after the aligned selected-region allocation. The fork schedule otherwise
/// pushes that allocation across an entire 8,192-row boundary. The legacy
/// profile keeps its original ordering and matrix.
#[must_use = "finish the allocation binding before completing the block relation"]
pub(crate) struct PreparedDevelopmentAllocation<'a> {
    trace: DevelopmentAllocationTrace,
    deferred: Option<(&'a LinExpr, &'a StateDepthTrace, &'a LinExpr, ForkSchedule)>,
}

impl<'a> PreparedDevelopmentAllocation<'a> {
    pub(crate) fn legacy(
        b: &mut FieldR1csBuilder,
        height: &'a LinExpr,
        depth: &'a StateDepthTrace,
        payout_raw_amount: &'a LinExpr,
    ) -> Self {
        let old = super::development_allocation::bind_development_allocation(
            b,
            height,
            depth,
            payout_raw_amount,
        );
        Self {
            trace: DevelopmentAllocationTrace {
                height_bits: old.height_bits,
                v2_active: LinExpr::zero(),
                active: old.active,
                payout_due: old.payout_due,
                share_each: old.share_each,
                miner_subsidy: old.miner_subsidy,
                payout_each: old.payout_each,
            },
            deferred: None,
        }
    }

    pub(crate) fn new(
        b: &mut FieldR1csBuilder,
        height: &'a LinExpr,
        depth: &'a StateDepthTrace,
        payout_raw_amount: &'a LinExpr,
        schedule: ForkSchedule,
    ) -> Self {
        assert!(schedule.v2().is_some());
        let tower = |expr: &LinExpr| {
            let flat = expr.eval(b.values());
            noid_core::hardware::flat_to_tower_u128((flat.lo as u128) | ((flat.hi as u128) << 64))
        };
        let native_height = u64::try_from(tower(height)).expect("child height fits u64");
        let native_depth = u32::try_from(tower(&depth.value)).expect("child depth fits u32");
        let native = development_allocation_with_schedule(native_height, native_depth, schedule)
            .expect("honest schedule");
        let height_bits = range_check_bits(b, height, HEIGHT_BITS).try_into().unwrap();
        let trace = DevelopmentAllocationTrace {
            height_bits,
            v2_active: alloc_block(
                b,
                Block128::from(u64::from(native_height >= schedule.v2().unwrap().height())),
            ),
            active: alloc_block(b, Block128::from(u64::from(native.active))),
            payout_due: alloc_block(b, Block128::from(u64::from(native.payout_due))),
            share_each: alloc_block(
                b,
                Block128::from(if native.active { native.share_each } else { 0 }),
            ),
            miner_subsidy: alloc_block(b, Block128::from(native.miner_subsidy)),
            payout_each: alloc_block(b, Block128::from(native.payout_each.unwrap_or(0))),
        };
        Self {
            trace,
            deferred: Some((height, depth, payout_raw_amount, schedule)),
        }
    }

    pub(crate) fn trace(&self) -> &DevelopmentAllocationTrace {
        &self.trace
    }

    pub(crate) fn finish(self, b: &mut FieldR1csBuilder) {
        let Some((height, depth, payout_raw, schedule)) = self.deferred else {
            return;
        };
        let bound =
            bind_development_allocation_with_schedule(b, height, depth, payout_raw, schedule.v2());
        for (reserved, actual) in [
            (&self.trace.v2_active, &bound.v2_active),
            (&self.trace.active, &bound.active),
            (&self.trace.payout_due, &bound.payout_due),
            (&self.trace.share_each, &bound.share_each),
            (&self.trace.miner_subsidy, &bound.miner_subsidy),
            (&self.trace.payout_each, &bound.payout_each),
        ] {
            pin_eq(b, reserved, actual);
        }
        // Both decompositions bind the same authenticated height. Keep the
        // original early bits for the contract policy; no detached height is
        // introduced by delaying the schedule arithmetic.
    }
}

fn shifted_integer_from_bits(bits: &[Wire], shift: usize) -> LinExpr {
    assert!(bits.len() + shift <= HEIGHT_BITS);
    bits.iter()
        .enumerate()
        .fold(LinExpr::zero(), |sum, (index, &wire)| {
            sum.add(&LinExpr::from_wire(wire).scale(flat_const(1u128 << (index + shift))))
        })
}

fn less_than_bits(b: &mut FieldR1csBuilder, lhs: &[LinExpr], rhs: &[LinExpr]) -> LinExpr {
    assert_eq!(lhs.len(), rhs.len());
    let mut borrow = LinExpr::zero();
    for (left, right) in lhs.iter().zip(rhs) {
        let left_zero_right_one = mul(b, &left.add_const(F128::ONE), right);
        let borrow_when_equal = mul(b, &borrow, &left.add(right).add_const(F128::ONE));
        borrow = left_zero_right_one.add(&borrow_when_equal);
    }
    borrow
}

fn constant_bits(value: u64, width: usize) -> Vec<LinExpr> {
    (0..width)
        .map(|bit| {
            if (value >> bit) & 1 == 1 {
                LinExpr::constant(F128::ONE)
            } else {
                LinExpr::zero()
            }
        })
        .collect()
}

fn selected_depth_constant(depth: &StateDepthTrace, values: &[u64]) -> LinExpr {
    assert_eq!(
        values.len(),
        MAX_EXACT_STATE_DEPTH - MIN_EXACT_STATE_DEPTH + 1
    );
    depth
        .one_hot
        .iter()
        .zip(values)
        .fold(LinExpr::zero(), |sum, (selector, value)| {
            sum.add(&selector.scale(flat_const(*value as u128)))
        })
}

fn payout_boundary_for(
    b: &mut FieldR1csBuilder,
    height: &LinExpr,
    native_height: u64,
    interval: u64,
) -> LinExpr {
    assert!(interval > 0);
    let shifts: Vec<usize> = (0..64)
        .rev()
        .filter(|shift| interval >> shift & 1 != 0)
        .collect();
    let remainder_bits_len = (64 - interval.leading_zeros()) as usize;
    let quotient = native_height / interval;
    let remainder = native_height % interval;
    let quotient = alloc_block(b, Block128::from(quotient as u128));
    let quotient_bits = range_check_bits(b, &quotient, HEIGHT_BITS - shifts[0]);
    let remainder = alloc_block(b, Block128::from(remainder as u128));
    let remainder_bits = range_check_bits(b, &remainder, remainder_bits_len);
    let divisor = const_block(Block128::from(interval as u128));
    let divisor_bits = range_check_bits(b, &divisor, remainder_bits_len);
    pin_lt_strict(b, &remainder_bits, &divisor_bits);

    let terms: Vec<_> = shifts
        .iter()
        .map(|&shift| shifted_integer_from_bits(&quotient_bits, shift))
        .collect();
    let mut product = terms[0].clone();
    for term in &terms[1..] {
        product = integer_add_no_overflow(b, &product, term, HEIGHT_BITS);
    }
    let recomposed = integer_add_no_overflow(b, &product, &remainder, HEIGHT_BITS);
    pin_eq(b, height, &recomposed);

    remainder_bits
        .iter()
        .fold(LinExpr::constant(F128::ONE), |zero, &bit| {
            mul(b, &zero, &LinExpr::from_wire(bit).add_const(F128::ONE))
        })
}

fn equals_constant(b: &mut FieldR1csBuilder, bits: &[LinExpr], value: u64) -> LinExpr {
    bits.iter()
        .enumerate()
        .fold(LinExpr::constant(F128::ONE), |equal, (index, bit)| {
            let matching = if value >> index & 1 == 1 {
                bit.clone()
            } else {
                bit.add_const(F128::ONE)
            };
            mul(b, &equal, &matching)
        })
}

/// Disjoint payout selectors with fixed block counts. The daily and final
/// intervals are constants of the frozen schedule, never chosen by a
/// prover. The output is independent of the occupied transaction positions.
fn payout_rules(
    b: &mut FieldR1csBuilder,
    height: &LinExpr,
    bits: &[LinExpr],
    native_height: u64,
    activation: Option<V2Activation>,
) -> (LinExpr, Vec<(LinExpr, u64)>) {
    let Some(activation) = activation else {
        let due = payout_boundary_for(b, height, native_height, TARGET_BLOCKS_PER_DAY);
        return (
            LinExpr::constant(F128::ZERO),
            vec![(due, TARGET_BLOCKS_PER_DAY)],
        );
    };
    let at = activation.height();
    assert_eq!(86_400 % activation.block_time(), 0);
    let interval = 86_400 / activation.block_time();
    let before = less_than_bits(b, bits, &constant_bits(at, HEIGHT_BITS));
    let after = before.add_const(F128::ONE);
    let old_boundary = payout_boundary_for(b, height, native_height, 4_320);
    let old_due = mul(b, &before, &old_boundary);
    let elapsed = alloc_block(b, Block128::from(native_height.saturating_sub(at)));
    let selected_height = mul(b, &after, height);
    let origin = after.scale(flat_const(at as u128));
    let recomposed = integer_add_no_overflow(b, &elapsed, &origin, HEIGHT_BITS);
    pin_eq(b, &selected_height, &recomposed);
    let boundary = payout_boundary_for(b, &elapsed, native_height.saturating_sub(at), interval);
    let first = equals_constant(b, bits, at);
    let regular = mul(b, &after, &first.add_const(F128::ONE));
    let regular = mul(b, &regular, &boundary);
    let mut rules = vec![(old_due, 4_320), (regular, interval)];
    let final_blocks = DEVELOPMENT_ALLOCATION_END_HEIGHT.saturating_sub(at) % interval;
    if final_blocks != 0 {
        rules.push((
            equals_constant(b, bits, DEVELOPMENT_ALLOCATION_END_HEIGHT),
            final_blocks,
        ));
    }
    (after, rules)
}

fn bind_development_allocation_with_schedule(
    b: &mut FieldR1csBuilder,
    child_height: &LinExpr,
    child_depth: &StateDepthTrace,
    payout_raw_amount: &LinExpr,
    activation: Option<V2Activation>,
) -> DevelopmentAllocationTrace {
    use noid_core::hardware::flat_to_tower_u128;

    let flat = child_height.eval(b.values());
    let tower = flat_to_tower_u128((flat.lo as u128) | ((flat.hi as u128) << 64));
    let native_height = u64::try_from(tower).expect("child height fits u64");
    let height_wires = range_check_bits(b, child_height, HEIGHT_BITS);
    let height_bits = height_wires
        .iter()
        .copied()
        .map(LinExpr::from_wire)
        .collect::<Vec<_>>();

    let below_end = less_than_bits(
        b,
        &height_bits,
        &constant_bits(DEVELOPMENT_ALLOCATION_END_HEIGHT + 1, HEIGHT_BITS),
    );
    let height_is_zero = height_bits
        .iter()
        .fold(LinExpr::constant(F128::ONE), |zero, bit| {
            mul(b, &zero, &bit.add_const(F128::ONE))
        });
    let active = mul(b, &below_end, &height_is_zero.add_const(F128::ONE));
    let (v2_active, payout_rules) =
        payout_rules(b, child_height, &height_bits, native_height, activation);
    let interval_boundary = payout_rules
        .iter()
        .fold(LinExpr::zero(), |sum, (selector, _)| sum.add(selector));
    let payout_due = mul(b, &active, &interval_boundary);

    let rewards = (MIN_EXACT_STATE_DEPTH..=MAX_EXACT_STATE_DEPTH)
        .map(|depth| noid_chain::consensus::emission::block_reward(depth as u32))
        .collect::<Vec<_>>();
    let shares = rewards.iter().map(|reward| reward / 20).collect::<Vec<_>>();
    let miner_active = rewards
        .iter()
        .zip(&shares)
        .map(|(reward, share)| reward - 2 * share)
        .collect::<Vec<_>>();
    let full_subsidy = selected_depth_constant(child_depth, &rewards);
    let share_each = selected_depth_constant(child_depth, &shares);
    let fork_schedule = activation.is_some();
    let selected_payout_each = if fork_schedule {
        payout_rules
            .iter()
            .fold(LinExpr::zero(), |sum, (selector, blocks)| {
                let amounts = shares
                    .iter()
                    .map(|share| share.checked_mul(*blocks).expect("payout fits u64"))
                    .collect::<Vec<_>>();
                sum.add(&mul(
                    b,
                    selector,
                    &selected_depth_constant(child_depth, &amounts),
                ))
            })
    } else {
        let amounts = shares
            .iter()
            .map(|share| {
                share
                    .checked_mul(TARGET_BLOCKS_PER_DAY)
                    .expect("payout fits u64")
            })
            .collect::<Vec<_>>();
        selected_depth_constant(child_depth, &amounts)
    };
    let active_miner = selected_depth_constant(child_depth, &miner_active);
    let miner_subsidy = full_subsidy.add(&mul(b, &active, &full_subsidy.add(&active_miner)));

    let current_share = mul(b, &active, &share_each);
    let expected_payout = mul(
        b,
        if fork_schedule { &active } else { &payout_due },
        &selected_payout_each,
    );
    // Suffix slot zero is a user body off payout heights and the payout body
    // on a boundary. Only the schedule-selected amount is monetary here.
    let selected_payout = mul(b, &payout_due, payout_raw_amount);
    pin_eq(b, &selected_payout, &expected_payout);

    let native_depth = {
        let flat = child_depth.value.eval(b.values());
        let tower = flat_to_tower_u128((flat.lo as u128) | ((flat.hi as u128) << 64));
        u32::try_from(tower).expect("state depth fits u32")
    };
    let schedule = ForkSchedule::new(Some(0), activation).expect("checked schedule");
    let native_payout = development_allocation_with_schedule(native_height, native_depth, schedule)
        .expect("checked candidate schedule")
        .payout_each
        .unwrap_or(0);
    debug_assert_eq!(
        selected_payout.eval(b.values()),
        alloc_block_value(native_payout)
    );

    DevelopmentAllocationTrace {
        height_bits: height_wires
            .try_into()
            .expect("child height range check has 64 bits"),
        v2_active,
        active,
        payout_due,
        share_each: current_share,
        miner_subsidy,
        payout_each: expected_payout,
    }
}

fn alloc_block_value(value: u64) -> F128 {
    use noid_core::hardware::tower_to_flat_u128;
    let flat = tower_to_flat_u128(value as u128);
    F128 {
        lo: flat as u64,
        hi: (flat >> 64) as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_schedule_matches_native_boundaries_and_rejects_changed_height() {
        for interval in [20, 30, 40] {
            let period = 86_400 / interval;
            for activation in [10, 4_320, DEVELOPMENT_ALLOCATION_END_HEIGHT, u64::MAX] {
                let at = V2Activation::new(activation, interval).unwrap();
                let schedule = ForkSchedule::new(Some(5), Some(at)).unwrap();
                let mut digest = None;
                for height in [
                    activation - 1,
                    activation,
                    activation.saturating_add(1),
                    activation.saturating_add(period),
                    DEVELOPMENT_ALLOCATION_END_HEIGHT,
                    DEVELOPMENT_ALLOCATION_END_HEIGHT + 1,
                    u64::MAX,
                ] {
                    let native =
                        development_allocation_with_schedule(height, 24, schedule).unwrap();
                    let mut b = FieldR1csBuilder::new();
                    let h = b.alloc_f128(alloc_block_value(height));
                    let h_expr = LinExpr::from_wire(h);
                    let depth_value = alloc_block(&mut b, Block128(24));
                    let depth = StateDepthTrace::bind(&mut b, &depth_value);
                    let amount =
                        alloc_block(&mut b, Block128(native.payout_each.unwrap_or(0) as u128));
                    let prepared = PreparedDevelopmentAllocation::new(
                        &mut b, &h_expr, &depth, &amount, schedule,
                    );
                    let trace = prepared.trace();
                    assert_eq!(
                        trace.payout_each.eval(b.values()),
                        alloc_block_value(native.payout_each.unwrap_or(0))
                    );
                    assert_eq!(
                        trace.miner_subsidy.eval(b.values()),
                        alloc_block_value(native.miner_subsidy)
                    );
                    prepared.finish(&mut b);
                    let (matrix, mut witness) = b.build();
                    assert!(
                        matrix.satisfies(&witness),
                        "interval={interval}, activation={activation}, height={height}"
                    );
                    let actual = matrix.structural_statement_digest();
                    assert!(digest.is_none_or(|expected| expected == actual));
                    digest = Some(actual);
                    witness[h.0 as usize] += F128::ONE;
                    assert!(!matrix.satisfies(&witness));
                }
            }
        }
    }
}
