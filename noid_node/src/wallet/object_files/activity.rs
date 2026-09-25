// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Bounded discovery of locally retained calls. Index records carry the small
//! opening/body/Merkle statement; browsing never opens a recursive terminal.

use super::*;
use noid_rpc::wallet_ops::{WalletObjectReceiptPage, WalletObjectReceiptRecord};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const INDEX: &str = "activity-v1";
const RECORD_LIMIT: usize = 16 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    opening: String,
    page: String,
    header: String,
    tx_index: u16,
    tx_count: u16,
    path: [[u8; 32]; noid_chain::tx_tree::TX_TREE_DEPTH],
}

fn family(opening: &ObjectOpening) -> String {
    hex::encode(opening.successor(Default::default()).root().0)
}

fn cursor(height: u64, txid: [u8; 32]) -> String {
    format!("{height:016x}-{}", hex::encode(txid))
}

fn valid_cursor(value: &str) -> bool {
    value.len() == 81
        && value.as_bytes()[16] == b'-'
        && value
            .bytes()
            .enumerate()
            .all(|(i, b)| i == 16 || b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(super) fn remember(key: &Path, receipt: &ObjectTransitionReceipt) -> Result<(), String> {
    let txid = noid_tx::hash_paged_spend(std::slice::from_ref(&receipt.page))
        .map_err(|e| e.to_string())?
        .0;
    let record = Record {
        opening: hex::encode(receipt.opening.to_bytes().map_err(|e| e.to_string())?),
        page: hex::encode(receipt.page.to_bytes().map_err(|e| e.to_string())?),
        header: hex::encode(receipt.header.to_bytes()),
        tx_index: receipt.tx_index,
        tx_count: receipt.tx_count,
        path: receipt.path,
    };
    let path = directory(key)?
        .join(INDEX)
        .join(family(&receipt.opening))
        .join(cursor(receipt.header.height, txid));
    let bytes = serde_json::to_vec(&record).map_err(|e| e.to_string())?;
    if !read_bounded(&path, RECORD_LIMIT).is_ok_and(|old| old == bytes) {
        write_atomic(&path, &bytes)?;
        // Persist a newly created family directory before writing a migration marker.
        #[cfg(unix)]
        std::fs::File::open(directory(key)?.join(INDEX))
            .and_then(|dir| dir.sync_all())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_ready(key: &Path) -> Result<(), String> {
    let objects = directory(key)?;
    let marker = objects.join(INDEX).join("ready");
    if read_bounded(&marker, 0).is_ok() {
        return Ok(());
    }
    std::fs::create_dir_all(&objects).map_err(|e| e.to_string())?;
    // One-time migration of existing receipts. A crash resumes idempotently.
    for entry in std::fs::read_dir(&objects).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_none_or(|ext| ext != "receipt") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.len() != 64 || !name.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let txid: [u8; 32] = hex::decode(name)
            .map_err(|e| e.to_string())?
            .try_into()
            .map_err(|_| "invalid receipt name")?;
        let receipt = ObjectTransitionReceipt::from_bytes(&load_receipt(key, txid)?)?;
        remember(key, &receipt)?;
    }
    write_atomic(&marker, &[])?;
    #[cfg(unix)]
    std::fs::File::open(&objects)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn read_record(
    path: &Path,
    expected_family: &str,
    name: &str,
) -> Result<WalletObjectReceiptRecord, String> {
    let stored: Record =
        serde_json::from_slice(&read_bounded(path, RECORD_LIMIT)?).map_err(|e| e.to_string())?;
    let opening =
        ObjectOpening::from_bytes(&hex::decode(stored.opening).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let page = noid_tx::TxPage::from_bytes(&hex::decode(stored.page).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let header = noid_chain::BlockHeader::from_bytes(
        &hex::decode(stored.header).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("receipt index header: {e:?}"))?;
    let txid = noid_tx::hash_paged_spend(std::slice::from_ref(&page))
        .map_err(|e| e.to_string())?
        .0;
    if family(&opening) != expected_family
        || cursor(header.height, txid) != name
        || !noid_chain::consensus::params::v2_active(header.height)
        || !noid_chain::tx_tree::verify_path(
            txid,
            &stored.path,
            usize::from(stored.tx_index),
            usize::from(stored.tx_count),
            header.tx_root,
        )
    {
        return Err("saved contract activity inclusion mismatch".into());
    }
    opening
        .check_call(&page, header.height)
        .map_err(|e| e.to_string())?;
    Ok(WalletObjectReceiptRecord {
        opening,
        page,
        header,
    })
}

/// Newest-first pagination. The directory walk retains O(limit) names and reads
/// at most limit compact records from this contract family, not full proofs.
pub(super) fn page(
    key: &Path,
    opening: &ObjectOpening,
    after: Option<&str>,
    limit: usize,
) -> Result<WalletObjectReceiptPage, String> {
    if !(1..=64).contains(&limit) {
        return Err("contract activity page must contain 1..64 calls".into());
    }
    if after.is_some_and(|value| !valid_cursor(value)) {
        return Err("invalid contract activity cursor".into());
    }
    opening.validate().map_err(|e| e.to_string())?;
    ensure_ready(key)?;
    let expected = family(opening);
    let bucket = directory(key)?.join(INDEX).join(&expected);
    let entries = match std::fs::read_dir(&bucket) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(WalletObjectReceiptPage {
                receipts: Vec::new(),
                next_cursor: None,
            })
        }
        Err(e) => return Err(e.to_string()),
    };
    let mut names = BTreeSet::new();
    for entry in entries {
        let name = entry.map_err(|e| e.to_string())?.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !valid_cursor(name) || after.is_some_and(|after| name >= after) {
            continue;
        }
        names.insert(name.to_owned());
        if names.len() > limit + 1 {
            names.pop_first();
        }
    }
    let more = names.len() > limit;
    if more {
        names.pop_first();
    }
    let next_cursor = more.then(|| names.first().unwrap().clone());
    let receipts = names
        .into_iter()
        .rev()
        .map(|name| read_record(&bucket.join(&name), &expected, &name))
        .collect::<Result<_, _>>()?;
    Ok(WalletObjectReceiptPage {
        receipts,
        next_cursor,
    })
}
