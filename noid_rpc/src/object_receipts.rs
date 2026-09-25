// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Receipt assembly from already accepted canonical storage. A suffix receiver
//! can use its later recursive terminal without fetching old intermediate proofs.

use noid_block::contract_receipt::{ObjectTransitionReceipt, MAX_RECEIPT_DESCENDANTS};
use noid_chain::{storage::MdbxStore, Block};
use noid_tx::experimental_object::ObjectOpening;

pub fn from_store(
    store: &MdbxStore,
    block: &Block,
    group: usize,
    opening: ObjectOpening,
) -> Result<Option<ObjectTransitionReceipt>, String> {
    if store
        .get_header(block.header.height)
        .map_err(|e| e.to_string())?
        != Some(block.header)
    {
        return Err("receipt body is no longer canonical".into());
    }
    let mut header = block.header;
    let mut descendants = Vec::new();
    for _ in 0..=MAX_RECEIPT_DESCENDANTS {
        if let Some(terminal) = store
            .get_history_step_terminal_at(header.height, noid_chain::block_id(&header))
            .map_err(|e| e.to_string())?
        {
            let epoch_height =
                noid_chain::consensus::tx_epoch_anchor_height_for_child(header.height);
            let epoch = store
                .get_header(epoch_height)
                .map_err(|e| e.to_string())?
                .ok_or("receipt proof epoch header missing")?;
            let mut receipt =
                ObjectTransitionReceipt::from_block(block, group, opening, epoch, terminal)?;
            receipt.descendants = descendants;
            receipt.proof_header()?;
            return Ok(Some(receipt));
        }
        if descendants.len() == MAX_RECEIPT_DESCENDANTS {
            break;
        }
        let Some(next_height) = header.height.checked_add(1) else {
            break;
        };
        let Some(next) = store.get_header(next_height).map_err(|e| e.to_string())? else {
            break;
        };
        if next.prev_block_hash != noid_chain::block_id(&header) {
            return Err("receipt canonical ancestry changed".into());
        }
        descendants.push(next);
        header = next;
    }
    Ok(None)
}
