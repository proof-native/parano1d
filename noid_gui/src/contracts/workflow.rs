// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

use super::{Info, Instances, Kind, VerifiedReceipt};
use serde::{Deserialize, Serialize};

pub const ACTIVITY_LIMIT: usize = 256;
pub const ACTIVITY_FILE_LIMIT: usize = 1024 * 1024;
pub const CONTRACT_FILE_LIMIT: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Created,
    Imported,
    #[default]
    Existing,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Self::Created => "CREATED HERE",
            Self::Imported => "FROM A FILE",
            Self::Existing => "SAVED IN THIS WALLET",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DetailTab {
    #[default]
    Actions,
    Activity,
    Rules,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UseAction {
    #[default]
    Fund,
    Pay,
    Close,
    Continue,
}

#[derive(Debug, Clone)]
pub struct Creation {
    pub name: String,
    pub kind: Kind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Funding,
    Payment,
    Withdrawal,
    Update,
}

impl OperationKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Funding => "DEPOSIT",
            Self::Payment => "PAYMENT",
            Self::Withdrawal => "WITHDRAWAL",
            Self::Update => "CONTRACT CALL",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Confirmation {
    pub height: u64,
    pub block_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub txid: String,
    pub opening_hex: String,
    pub address: String,
    pub kind: OperationKind,
    pub amount_micronoid: Option<u64>,
    pub fee_micronoid: Option<u64>,
    #[serde(default)]
    pub authority: Option<String>,
    #[serde(default)]
    pub recipient: Option<String>,
    pub call_height: Option<u64>,
    pub confirmation: Option<Confirmation>,
    // Reloading a local record never establishes current chain selection.
    #[serde(skip)]
    pub canonical: bool,
    #[serde(skip)]
    pub receipt_available: bool,
}

impl Operation {
    pub fn valid(&self) -> bool {
        let hex = |s: &str, len| s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit());
        hex(&self.txid, 64)
            && self.opening_hex.len() <= 4096
            && !self.opening_hex.is_empty()
            && self.opening_hex.bytes().all(|b| b.is_ascii_hexdigit())
            && self.address.len() <= 128
            && self.authority.as_ref().is_none_or(|v| v.len() <= 128)
            && self.recipient.as_ref().is_none_or(|v| v.len() <= 128)
            && self
                .confirmation
                .as_ref()
                .is_none_or(|c| hex(&c.block_hash, 64))
    }

    pub fn status(&self, height: u64) -> &'static str {
        if self.canonical {
            "CONFIRMED"
        } else if self.confirmation.is_some() {
            "CHAIN CHANGED"
        } else if self.call_height.is_some_and(|at| height >= at) {
            "NOT CONFIRMED — REVIEW AGAIN"
        } else {
            "AWAITING CONFIRMATION"
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptKind {
    Funding,
    Call,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedProof {
    pub kind: ReceiptKind,
    pub receipt_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractFile {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub opening_hex: String,
    pub proof: Option<SharedProof>,
}

#[derive(Debug, Clone)]
pub struct OpenedFile {
    pub file_name: String,
    pub name: String,
    pub info: Option<Info>,
    pub instances: Option<Instances>,
    pub proof: Option<SharedProof>,
    pub verified_call: Option<VerifiedReceipt>,
    pub operation: Option<Operation>,
}

#[derive(Debug, Deserialize)]
pub struct RetainedCall {
    #[serde(flatten)]
    pub call: VerifiedReceipt,
    pub block_hash: String,
    pub canonical: bool,
}

#[derive(Debug, Deserialize)]
pub struct RetainedCalls {
    pub height: u64,
    pub tip_hash: String,
    pub entries: Vec<RetainedCall>,
    pub next_cursor: Option<String>,
}
