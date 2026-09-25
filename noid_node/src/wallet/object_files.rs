// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Durable public openings and receipts. These files never contain wallet
//! keys. Replacement is atomic, and every reload is bounded and revalidated.

use noid_block::contract_receipt::{ObjectTransitionReceipt, MAX_RECEIPT_BYTES};
use noid_tx::experimental_object::{ObjectOpening, OPENING_BYTES};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn directory(key_path: &Path) -> Result<PathBuf, String> {
    Ok(key_path
        .parent()
        .ok_or("wallet directory missing")?
        .join("objects"))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("object artifact directory missing")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let (temporary, mut file) = loop {
        let temporary = path.with_extension(format!(
            "{}.{}.partial",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temporary) {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    };
    let result = (|| {
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        std::fs::File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err("object artifact file bound".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("object artifact grew beyond bound".into());
    }
    Ok(bytes)
}

pub(super) fn save_opening(key_path: &Path, opening: &ObjectOpening) -> Result<(), String> {
    let path = directory(key_path)?.join(format!("{}.opening", hex::encode(opening.root().0)));
    write_atomic(&path, &opening.to_bytes().map_err(|e| e.to_string())?)
}

pub(super) fn load_opening(key_path: &Path, root: [u8; 32]) -> Result<ObjectOpening, String> {
    let path = directory(key_path)?.join(format!("{}.opening", hex::encode(root)));
    let opening = ObjectOpening::from_bytes(&read_bounded(&path, OPENING_BYTES)?)
        .map_err(|e| e.to_string())?;
    if opening.root().0 != root {
        return Err("saved opening root mismatch".into());
    }
    Ok(opening)
}

pub(super) fn save_receipt(key_path: &Path, txid: [u8; 32], bytes: &[u8]) -> Result<(), String> {
    let receipt = ObjectTransitionReceipt::from_bytes(bytes)?;
    if noid_tx::hash_paged_spend(std::slice::from_ref(&receipt.page))
        .map_err(|e| e.to_string())?
        .0
        != txid
    {
        return Err("object receipt transaction mismatch".into());
    }
    write_atomic(
        &directory(key_path)?.join(format!("{}.receipt", hex::encode(txid))),
        bytes,
    )
}

pub(super) fn load_receipt(key_path: &Path, txid: [u8; 32]) -> Result<Vec<u8>, String> {
    let bytes = read_bounded(
        &directory(key_path)?.join(format!("{}.receipt", hex::encode(txid))),
        MAX_RECEIPT_BYTES,
    )?;
    let receipt = ObjectTransitionReceipt::from_bytes(&bytes)?;
    if noid_tx::hash_paged_spend(std::slice::from_ref(&receipt.page))
        .map_err(|e| e.to_string())?
        .0
        != txid
    {
        return Err("saved object receipt transaction mismatch".into());
    }
    Ok(bytes)
}

/// Recover the complete bounded serving window after a suffix is committed.
/// Earlier intermediate blocks can lack an individual terminal until its last
/// block arrives; replaying only that last body would lose their receipts.
pub(super) fn retain_from_chain(
    key_path: &Path,
    chain: &noid_chain::storage::MdbxChainContext,
) -> Result<usize, String> {
    let tip = chain.tip_height();
    if !noid_chain::consensus::params::v2_active(tip) || !directory(key_path)?.is_dir() {
        return Ok(0);
    }
    let first = tip
        .saturating_sub(
            noid_chain::consensus::params::RETAINED_BLOCK_SERVING_DEPTH.saturating_sub(1),
        )
        .max(noid_chain::consensus::params::V2_ACTIVATION_HEIGHT.unwrap());
    let mut retained = 0;
    for height in first..=tip {
        if let Some(bytes) = chain
            .store
            .get_recent_canonical_block(height)
            .map_err(|e| e.to_string())?
        {
            let block = noid_chain::Block::from_bytes(&bytes)
                .map_err(|e| format!("retained contract block: {e:?}"))?;
            retained += retain_from_block(key_path, &chain.store, &block)?;
        }
    }
    Ok(retained)
}

/// Capture receipts for every locally retained opening, including calls made
/// by its other authority. Public artifacts survive account switches and can
/// be recovered from the bounded body window after an interrupted wallet fsync.
pub(super) fn retain_from_block(
    key_path: &Path,
    store: &noid_chain::storage::MdbxStore,
    block: &noid_chain::Block,
) -> Result<usize, String> {
    if !noid_chain::consensus::params::v2_active(block.header.height)
        || !directory(key_path)?.is_dir()
    {
        return Ok(0);
    }
    let stream =
        noid_chain::validate_block_page_stream(&block.transactions).map_err(|e| e.to_string())?;
    let mut retained = 0;
    for (group_index, group) in stream.groups.iter().enumerate() {
        let body = &block.transactions[stream.user_body_start(usize::from(group.start_page))].body;
        if body.validity_bitmap & noid_tx::PAGED_SPEND_CONTRACT_BIT == 0 {
            break;
        }
        let root = group.spend.input_owner.0;
        if !directory(key_path)?
            .join(format!("{}.opening", hex::encode(root)))
            .exists()
        {
            continue;
        }
        let opening = load_opening(key_path, root)?;
        let txid = group.spend.logical_txid.0;
        if load_receipt(key_path, txid)
            .ok()
            .and_then(|bytes| ObjectTransitionReceipt::from_bytes(&bytes).ok())
            .is_some_and(|receipt| receipt.header == block.header && receipt.opening == opening)
        {
            continue;
        }
        let Some(receipt) =
            noid_rpc::object_receipts::from_store(store, block, group_index, opening)?
        else {
            continue;
        };
        let checked = receipt
            .opening
            .check_call(&receipt.page, receipt.header.height)
            .map_err(|e| e.to_string())?;
        if !checked.terminal {
            save_opening(key_path, &receipt.opening.successor(checked.next))?;
        }
        save_receipt(key_path, txid, &receipt.to_bytes()?)?;
        retained += 1;
    }
    Ok(retained)
}

#[cfg(test)]
mod tests {
    use super::*;
    use noid_poseidon2b::primitives::Address;

    #[test]
    fn durable_openings_are_bounded_and_bound_to_their_filename() {
        let dir = tempfile::tempdir().unwrap();
        let key = dir.path().join("wallet.key");
        let opening =
            noid_tx::experimental_object::applications::timelocked_vault(Address([7; 32]), 20, 3);
        save_opening(&key, &opening).unwrap();
        assert_eq!(load_opening(&key, opening.root().0).unwrap(), opening);
        let path = directory(&key)
            .unwrap()
            .join(format!("{}.opening", hex::encode(opening.root().0)));
        let mut other = opening.clone();
        other.deadline += 1;
        std::fs::write(&path, other.to_bytes().unwrap()).unwrap();
        assert!(load_opening(&key, opening.root().0)
            .unwrap_err()
            .contains("root mismatch"));
        std::fs::write(&path, vec![0; OPENING_BYTES + 1]).unwrap();
        assert!(load_opening(&key, opening.root().0)
            .unwrap_err()
            .contains("bound"));
        save_opening(&key, &opening).unwrap();
        assert_eq!(load_opening(&key, opening.root().0).unwrap(), opening);
        assert!(std::fs::read_dir(directory(&key).unwrap())
            .unwrap()
            .all(|entry| !entry.unwrap().path().to_string_lossy().ends_with("partial")));
    }

    #[cfg(unix)]
    #[test]
    fn artifact_reload_rejects_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let link = dir.path().join("link");
        std::fs::write(&target, [0; 3]).unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(read_bounded(&link, 3).is_err());
    }
}
