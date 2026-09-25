// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

use noid_tx::experimental_object::integer_program::{Opcode, Operand, Predicate, Register};
use serde::{Deserialize, Serialize};

/// Exact unsigned arithmetic value. JSON uses canonical decimal strings so a
/// browser cannot silently round counters or instruction constants above 2^53.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectInteger(pub u64);

impl Serialize for ObjectInteger {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ObjectInteger {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.is_empty()
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(serde::de::Error::custom(
                "expected a canonical decimal u64 string",
            ));
        }
        value
            .parse::<u64>()
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

/// Constructors for ordinary policies. Custom openings use the same public
/// ABI and admission path, without an application-name consensus registry.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObjectDefinition {
    RefundablePayment {
        payer: String,
        payee: String,
        expiry_height: u64,
        max_fee_micronoid: u64,
    },
    TimelockedVault {
        owner: String,
        unlock_height: u64,
        max_fee_micronoid: u64,
    },
    AllowanceWallet {
        spending_key: String,
        recovery_key: String,
        payout_recipient: Option<String>,
        recover_at: u64,
        max_fee_micronoid: u64,
        max_payout_micronoid: u64,
        min_retained_micronoid: u64,
    },
    PeriodBudgetWallet {
        spending_key: String,
        recovery_key: String,
        payout_recipient: Option<String>,
        start_height: u64,
        period_blocks: u64,
        budget_micronoid: u64,
        recover_at: u64,
        max_fee_micronoid: u64,
        max_payout_micronoid: u64,
        min_retained_micronoid: u64,
    },
    RecurringPayment {
        payer: String,
        payee: String,
        first_due_height: u64,
        period_blocks: u64,
        payment_micronoid: u64,
        recover_at: u64,
        max_fee_micronoid: u64,
    },
    TrancheVesting {
        beneficiary: String,
        first_unlock_height: u64,
        period_blocks: u64,
        tranche_micronoid: u64,
        mature_at: u64,
        max_fee_micronoid: u64,
    },
    CustomProgram {
        definition: ObjectProgramDefinition,
    },
    Custom {
        opening_hex: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectProgramStep {
    pub opcode: Opcode,
    pub destination: Register,
    pub left: Operand,
    pub right: Operand,
    pub predicate: Predicate,
    pub immediate: ObjectInteger,
}

/// A program with explicit policies. Empty trailing instructions are padded
/// with canonical Keep operations. There are no implicit authorities or modes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectProgramDefinition {
    pub state: [ObjectInteger; 2],
    pub program: Vec<ObjectProgramStep>,
    pub claim_authority: String,
    pub recovery_authority: String,
    pub claim_recipient: String,
    pub recovery_recipient: String,
    pub deadline_height: u64,
    pub max_fee_micronoid: u64,
    pub max_payout_micronoid: u64,
    pub min_retained_micronoid: u64,
    pub claim_can_continue: bool,
    pub claim_can_close: bool,
    pub recovery_can_continue: bool,
    pub recovery_can_close: bool,
    pub unrestricted_payout_recipient: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectInfo {
    pub abi_version: u16,
    pub state: [ObjectInteger; 2],
    pub address: String,
    pub opening_hex: String,
    pub code_id: String,
    pub state_hex: String,
    pub program: [ObjectProgramStep; noid_tx::experimental_object::PROGRAM_STEPS],
    pub claim_authority: String,
    pub recovery_authority: String,
    pub claim_recipient: String,
    pub recovery_recipient: String,
    pub deadline_height: u64,
    pub max_fee_micronoid: u64,
    pub max_payout_micronoid: u64,
    pub min_retained_micronoid: u64,
    pub claim_can_continue: bool,
    pub claim_can_close: bool,
    pub recovery_can_continue: bool,
    pub recovery_can_close: bool,
    pub unrestricted_payout_recipient: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectStatus {
    pub object: ObjectInfo,
    pub slot: crate::types::SlotInfo,
    pub matches_opening: bool,
    pub tip_height: u64,
    pub next_call_height: u64,
    pub active_authority: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectInstances {
    pub address: String,
    pub height: u64,
    pub tip_hash: String,
    pub slots: Vec<crate::types::SlotInfo>,
    /// Inclusive cursor for the next page; pages identify their own exact tip.
    pub next_slot: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectPayout {
    pub address: String,
    pub amount_micronoid: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectCallRequest {
    pub opening_hex: String,
    pub slot_index: u32,
    /// Exact incarnation is mandatory. A reused slot is never selected silently.
    pub creation_id: u64,
    pub terminal: bool,
    pub payout: Option<ObjectPayout>,
    /// Zero requests the live relay minimum; it must still fit the policy cap.
    pub fee_micronoid: u64,
    /// Optional wallet review guard, checked against the same locked tip used
    /// to build the call. This is client intent, not a new consensus field.
    #[serde(default)]
    pub expected_recovery: Option<bool>,
    /// When supplied, the active wallet must still be the reviewed key.
    #[serde(default)]
    pub expected_authority: Option<String>,
    /// Optional exact-height review guard. A changed height requires a fresh
    /// preview and wallet authorization, never an edit of the signed body.
    #[serde(default)]
    pub expected_call_height: Option<u64>,
    /// Binds a preview's exact transaction body, including allocated output
    /// positions, calculated fee and successor. Changing it needs new review.
    #[serde(default)]
    pub expected_txid: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectCallPreview {
    pub txid: String,
    pub call_height: u64,
    pub authority: String,
    pub recovery: bool,
    pub terminal: bool,
    pub fee_micronoid: u64,
    pub retained_micronoid: u64,
    pub payout: Option<ObjectPayout>,
    pub successor: Option<ObjectInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectCallResult {
    pub transaction: crate::types::WalletSendResult,
    pub call_height: u64,
    pub successor: Option<ObjectInfo>,
    pub output_slot: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectReceiptResult {
    pub valid: bool,
    pub height: u64,
    pub txid: String,
    pub terminal: bool,
    pub authority: String,
    pub successor: Option<ObjectInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectClassLimits {
    pub class: String,
    pub pages: usize,
    pub live_inputs: usize,
    pub contract_calls: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectProtocolInfo {
    pub tip_height: u64,
    pub activation_height: Option<u64>,
    pub active_at_next_block: bool,
    pub runtime_available: bool,
    pub next_block_time_seconds: u64,
    pub abi_version: u16,
    pub instructions: usize,
    pub persistent_registers: usize,
    pub classes: Vec<ObjectClassLimits>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_json_preserves_values_beyond_browser_number_precision() {
        for value in [0, 1, (1 << 53) + 1, u64::MAX] {
            let json = serde_json::to_string(&ObjectInteger(value)).unwrap();
            assert_eq!(json, format!("\"{value}\""));
            assert_eq!(
                serde_json::from_str::<ObjectInteger>(&json).unwrap().0,
                value
            );
        }
        for invalid in [
            "0",
            "9007199254740993",
            "null",
            "\"\"",
            "\"00\"",
            "\"01\"",
            "\"+1\"",
            "\"-1\"",
            "\" 1\"",
            "\"1.0\"",
            "\"1e2\"",
            "\"18446744073709551616\"",
        ] {
            assert!(
                serde_json::from_str::<ObjectInteger>(invalid).is_err(),
                "{invalid}"
            );
        }
    }
}

impl From<&ObjectInfo> for ObjectProgramDefinition {
    fn from(info: &ObjectInfo) -> Self {
        Self {
            state: info.state,
            program: info.program.to_vec(),
            claim_authority: info.claim_authority.clone(),
            recovery_authority: info.recovery_authority.clone(),
            claim_recipient: info.claim_recipient.clone(),
            recovery_recipient: info.recovery_recipient.clone(),
            deadline_height: info.deadline_height,
            max_fee_micronoid: info.max_fee_micronoid,
            max_payout_micronoid: info.max_payout_micronoid,
            min_retained_micronoid: info.min_retained_micronoid,
            claim_can_continue: info.claim_can_continue,
            claim_can_close: info.claim_can_close,
            recovery_can_continue: info.recovery_can_continue,
            recovery_can_close: info.recovery_can_close,
            unrestricted_payout_recipient: info.unrestricted_payout_recipient,
        }
    }
}
