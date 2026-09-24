// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Reusable application constructors. These are ordinary object programs and
//! policies; consensus has no application-name registry or deployment allowlist.

use noid_core::Block128;
use noid_poseidon2b::primitives::Address;

use super::{policy::*, ObjectOpening, PROGRAM_STEPS};

const KEEP_STATE: [[Block128; 2]; PROGRAM_STEPS] = [[Block128(0); 2]; PROGRAM_STEPS];

/// The payee may collect before expiry; the payer may recover at or after it.
/// No continuing call can drain the payment, and its closing fee is capped.
pub fn refundable_payment(
    payer: Address,
    payee: Address,
    expiry_height: u64,
    max_fee: u64,
) -> ObjectOpening {
    ObjectOpening {
        program: KEEP_STATE,
        state: Block128(0),
        claim_authority: payee,
        recovery_authority: payer,
        deadline: expiry_height,
        claim_recipient: payee,
        recovery_recipient: payer,
        rules: ObjectRules {
            max_fee,
            min_retained: 0,
            max_payout: 0,
            modes: CLAIM_CLOSE | RECOVERY_CLOSE,
        },
    }
}

/// Even the owner's correct authorization cannot spend before the unlock
/// height. Withdrawal then returns the balance, minus a capped fee, to owner.
pub fn timelocked_vault(owner: Address, unlock_height: u64, max_fee: u64) -> ObjectOpening {
    ObjectOpening {
        program: KEEP_STATE,
        state: Block128(0),
        claim_authority: owner,
        recovery_authority: owner,
        deadline: unlock_height,
        claim_recipient: owner,
        recovery_recipient: owner,
        rules: ObjectRules {
            max_fee,
            min_retained: 0,
            max_payout: 0,
            modes: RECOVERY_CLOSE,
        },
    }
}

/// A spending key can make bounded payments while preserving a reserve. It
/// cannot close the object. At expiry only the recovery key can withdraw the
/// remainder. The limit is per call, not per day or per key lifetime.
#[allow(clippy::too_many_arguments)]
pub fn allowance_wallet(
    spending_key: Address,
    recovery_key: Address,
    payout_recipient: Option<Address>,
    recover_at: u64,
    max_fee: u64,
    max_payout: u64,
    min_retained: u64,
) -> ObjectOpening {
    ObjectOpening {
        program: KEEP_STATE,
        state: Block128(0),
        claim_authority: spending_key,
        recovery_authority: recovery_key,
        deadline: recover_at,
        claim_recipient: payout_recipient.unwrap_or(spending_key),
        recovery_recipient: recovery_key,
        rules: ObjectRules {
            max_fee,
            min_retained,
            max_payout,
            modes: CLAIM_CONTINUE
                | RECOVERY_CLOSE
                | if payout_recipient.is_none() {
                    ANY_PAYOUT_RECIPIENT
                } else {
                    0
                },
        },
    }
}
