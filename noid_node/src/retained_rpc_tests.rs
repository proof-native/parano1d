// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Regression coverage for retained bodies with recursive-suffix markers.
//! Fixtures reproduce the durable read layout, not proof verification or State
//! execution. The chain crate separately tests the real suffix commit path.

use std::sync::{Arc, Mutex};

use libmdbx::{Database, DatabaseOptions, Mode, NoWriteMap, WriteFlags};
use noid_chain::storage::{encode_header, u64_key, MdbxChainContext, MdbxStore};
use noid_chain::{block_id, Block};
use noid_poseidon2b::primitives::Address;
use noid_tx::{
    output_bitmap_bit, Transaction, TxBody, TxInput, TxOutput, PAGED_SPEND_END_BIT,
    PAGED_SPEND_START_BIT, TX_INPUTS, TX_OUTPUTS,
};
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::sync::RwLock;

use crate::wallet::{reconcile_receipts_at_startup, state::WalletState, WalletHandle};

fn retained_suffix_fixture(owner: Address) -> (TempDir, MdbxChainContext, Block, Block) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join("genesis")).unwrap();
    let mut context = MdbxChainContext::open_or_create(&directory.path().join("genesis")).unwrap();
    let path = directory.path().join("retained");
    std::fs::create_dir_all(&path).unwrap();
    drop(MdbxStore::open(&path).unwrap());

    let genesis = *context.tip_header();
    let mut parent = genesis;
    let mut headers = vec![genesis];
    let mut bodies = Vec::new();
    for height in 1..=44 {
        if height < 43 {
            let mut header = parent;
            header.height = height;
            header.prev_block_hash = block_id(&parent);
            header.timestamp += 20;
            header.alloc_counter += 1;
            headers.push(header);
            parent = header;
            continue;
        }
        let mut outputs = [TxOutput::dummy(); TX_OUTPUTS];
        outputs[0] = TxOutput {
            slot_index: 100 + height as u32,
            amount: 50,
            owner: Address([7; 32]),
        };
        let coinbase = Transaction::new(TxBody {
            epoch_anchor: block_id(&parent),
            fee: 0,
            input_owner: Address([0; 32]),
            inputs: [TxInput::dummy(); TX_INPUTS],
            outputs,
            validity_bitmap: output_bitmap_bit(0),
            is_coinbase: true,
        });
        let mut transactions = vec![coinbase];
        if height == 43 {
            let mut payout = transactions[0].body.clone();
            payout.outputs = [
                TxOutput {
                    slot_index: 200,
                    amount: 2,
                    owner: Address([8; 32]),
                },
                TxOutput {
                    slot_index: 201,
                    amount: 3,
                    owner: Address([9; 32]),
                },
            ];
            payout.validity_bitmap |= output_bitmap_bit(1);
            transactions.push(Transaction::new(payout));
            let mut inputs = [TxInput::dummy(); TX_INPUTS];
            inputs[0] = TxInput {
                slot_index: 7,
                amount: 50,
                creation_id: 3,
            };
            let mut outputs = [TxOutput::dummy(); TX_OUTPUTS];
            outputs[0] = TxOutput {
                slot_index: 9,
                amount: 40,
                owner: Address([0xA5; 32]),
            };
            transactions.push(Transaction::new(TxBody {
                epoch_anchor: block_id(&genesis),
                fee: 10,
                input_owner: owner,
                inputs,
                outputs,
                validity_bitmap: 1
                    | output_bitmap_bit(0)
                    | PAGED_SPEND_START_BIT
                    | PAGED_SPEND_END_BIT,
                is_coinbase: false,
            }));
        }
        let mut header = parent;
        header.height = height;
        header.prev_block_hash = block_id(&parent);
        header.timestamp += 20;
        header.tx_root = noid_chain::compute_tx_root(&transactions);
        header.alloc_counter += transactions
            .iter()
            .map(|tx| tx.body.live_outputs().count() as u64)
            .sum::<u64>();
        headers.push(header);
        parent = header;
        if height >= 43 {
            bodies.push(Block {
                header,
                transactions,
            });
        }
    }
    let marker_block = bodies.remove(0);
    let tip_block = bodies.remove(0);
    let terminal_prefix = |block: &Block| {
        noid_chain::history_step::HistoryStepTerminalMetadata::new(
            block.header.height,
            noid_chain::block_header::semantic_header_id(&block.header),
            0,
        )
        .unwrap()
        .encode_prefix()
        .to_vec()
    };
    let mut marker = terminal_prefix(&marker_block);
    marker.extend_from_slice(b"RSM1");
    marker.extend_from_slice(&tip_block.header.height.to_le_bytes());
    marker.extend_from_slice(&block_id(&tip_block.header));
    let mut terminal = terminal_prefix(&tip_block);
    terminal.push(1);

    let db = Database::<NoWriteMap>::open_with_options(
        &path,
        DatabaseOptions {
            max_tables: Some(32),
            mode: Mode::ReadWrite(Default::default()),
            ..Default::default()
        },
    )
    .unwrap();
    let txn = db.begin_rw_txn().unwrap();
    let table = txn.open_table(Some("headers")).unwrap();
    for header in &headers {
        txn.put(
            &table,
            u64_key(header.height),
            encode_header(header),
            WriteFlags::empty(),
        )
        .unwrap();
    }
    let recent = txn.open_table(Some("recent")).unwrap();
    let terminals = txn.open_table(Some("history_step_terminals")).unwrap();
    for (block, proof) in [(&marker_block, marker), (&tip_block, terminal)] {
        txn.put(
            &recent,
            u64_key(block.header.height),
            block.to_bytes(),
            WriteFlags::empty(),
        )
        .unwrap();
        txn.put(
            &terminals,
            u64_key(block.header.height),
            proof,
            WriteFlags::empty(),
        )
        .unwrap();
    }
    txn.commit().unwrap();
    drop(db);
    context.store = MdbxStore::open(&path).unwrap();
    context.tip_height = tip_block.header.height;
    context.tip_hash = block_id(&tip_block.header);
    context.recent_headers = headers
        .into_iter()
        .map(|header| (header.height, header))
        .collect();
    assert!(context.store.get_recent_block(43).unwrap().is_some());
    assert!(context
        .store
        .get_recent_accepted_block_bundle_bounded(43)
        .unwrap()
        .is_none());
    assert!(context
        .store
        .get_recent_accepted_block_bundle_bounded(44)
        .unwrap()
        .is_some());
    (directory, context, marker_block, tip_block)
}

#[test]
fn startup_receipt_recovery_includes_recursive_suffix_marker_bodies() {
    use noid_chain::consensus::receipt::{verify_against_header, ParanoidReceipt};

    let wallet_directory = tempfile::tempdir().unwrap();
    let wallet_path = wallet_directory.path().join("wallet.key");
    let mut wallet = WalletState::create_or_load(wallet_path.clone()).unwrap();
    let (_directory, chain, marker_block, _) = retained_suffix_fixture(wallet.active_address());
    let txid = noid_chain::try_compute_logical_txids(&marker_block.transactions).unwrap()[2].0;

    assert!(wallet.history.is_empty());
    assert!(wallet.receipts.is_empty());
    assert_eq!(
        reconcile_receipts_at_startup(&mut wallet, &chain).unwrap(),
        (0, 1)
    );
    let receipt = ParanoidReceipt::from_bytes(&wallet.receipts[&txid]).unwrap();
    assert!(verify_against_header(&receipt, &marker_block.header));
    assert_eq!(receipt.summary.logical_txid, txid);
    assert_eq!(
        reconcile_receipts_at_startup(&mut wallet, &chain).unwrap(),
        (0, 0)
    );
    drop(wallet);
    let reloaded = WalletState::create_or_load(wallet_path).unwrap();
    assert!(reloaded.receipts.contains_key(&txid));
}

async fn rpc(client: &reqwest::Client, url: &str, method: &str, params: Value) -> Value {
    let response: Value = client
        .post(url)
        .json(&json!({
            "jsonrpc": "2.0", "id": 1, "method": method, "params": params
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(response.get("error").is_none(), "{response}");
    response["result"].clone()
}

#[tokio::test]
async fn rpc_retained_marker_body_matches_raw_block_and_recent_transactions() {
    let owner = Address([0x33; 32]);
    let (directory, context, marker_block, tip_block) = retained_suffix_fixture(owner);
    let mempool = noid_mempool::AsyncMempool::new(
        noid_mempool::ChainView::from_mdbx(&context),
        noid_mempool::MempoolConfig::default(),
    );
    let tip = noid_p2p::object_protocol::ChainPoint::new(context.tip_height(), context.tip_hash());
    let chain = Arc::new(RwLock::new(context));
    let (network, task) = noid_p2p::P2PNetwork::start(
        "/ip4/127.0.0.1/tcp/0".parse().unwrap(),
        vec![],
        chain.clone(),
        mempool.clone(),
        noid_p2p::NetworkTopics::for_network_cfg(&noid_chain::consensus::NetworkConfig::mainnet()),
        [0; 32],
        directory.path().to_path_buf(),
        noid_p2p::BackgroundCapacity::Full,
    )
    .unwrap();
    // Only the command handle is needed. No P2P task or peer runs in this RPC test.
    task.abort();
    let _ = task.await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let (server, _stop) = noid_rpc::start_rpc_server(
        address,
        chain,
        mempool,
        WalletHandle::new(Arc::new(Mutex::new(None))),
        Arc::new(tokio::sync::Mutex::new(())),
        network.cmd_tx.clone(),
        tokio::sync::watch::channel(noid_p2p::P2PHealthSnapshot::default()).1,
        tokio::sync::watch::channel(tip).0,
        tokio::sync::watch::channel(true).1,
        tokio::sync::watch::channel(noid_rpc::types::NodeSyncStage::Tip).1,
        tokio::sync::watch::channel(false).1,
        tokio::sync::watch::channel(0).1,
        0,
        true,
        false,
        "test".into(),
        1,
        1,
        None,
        None,
        noid_rpc::ExternalMiningAttemptInvalidator::new(),
        tokio::sync::broadcast::channel(1).0,
        false,
        None,
        None,
        None,
        false,
    )
    .await
    .unwrap();
    let url = format!("http://{address}");
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();

    let raw = rpc(&client, &url, "paranoid_getBlock", json!([43])).await;
    assert_eq!(raw, hex::encode(marker_block.to_bytes()));
    let details = rpc(&client, &url, "paranoid_getBlockDetails", json!([43])).await;
    let retained = &details["retained"];
    assert!(
        !retained.is_null(),
        "a retained marker body must be visible: {details}"
    );
    assert_eq!(retained["block_bytes"], marker_block.to_bytes().len());
    assert_eq!(retained["history_step_bytes"], 0);
    assert_eq!(retained["bundle_bytes"], 0);
    if noid_chain::consensus::params::v2_active(43) {
        assert_eq!(retained["proof_class"], "v2 / class unavailable");
    }
    assert_eq!(retained["transactions"].as_array().unwrap().len(), 3);
    assert_eq!(retained["transactions"][1]["development_payout"], true);
    let txids = noid_chain::try_compute_logical_txids(&marker_block.transactions).unwrap();
    let txid = hex::encode(txids[2].0);
    assert_eq!(retained["transactions"][2]["txid"], txid);
    assert_eq!(
        retained["transactions"][2]["outputs"][0]["amount_micronoid"],
        40
    );

    let details = rpc(&client, &url, "paranoid_getBlockDetails", json!([44])).await;
    assert_eq!(
        details["retained"]["block_bytes"],
        tip_block.to_bytes().len()
    );
    assert!(details["retained"]["history_step_bytes"].as_u64().unwrap() > 0);
    if noid_chain::consensus::params::v2_active(44) {
        assert_eq!(
            details["retained"]["proof_class"],
            "v2 / Small / parameters unavailable"
        );
    }
    assert!(
        details["retained"]["bundle_bytes"].as_u64().unwrap() > tip_block.to_bytes().len() as u64
    );
    assert!(rpc(&client, &url, "paranoid_getBlock", json!([1]))
        .await
        .is_null());
    let pruned = rpc(&client, &url, "paranoid_getBlockDetails", json!([1])).await;
    assert_eq!(pruned["header"]["height"], 1);
    assert!(pruned["retained"].is_null());
    assert!(rpc(&client, &url, "paranoid_getBlockDetails", json!([45]))
        .await
        .is_null());

    let recent = rpc(
        &client,
        &url,
        "paranoid_getRecentTransactions",
        json!([1, 32, null]),
    )
    .await;
    assert_eq!(recent["total"], 4);
    assert!(recent["transactions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tx| tx["txid"] == txid));
    let filtered = rpc(
        &client,
        &url,
        "paranoid_getRecentTransactions",
        json!([1, 1, owner.to_bech32()]),
    )
    .await;
    assert_eq!(filtered["total"], 1);
    assert_eq!(filtered["transactions"][0]["txid"], txid);
    let second_page = rpc(
        &client,
        &url,
        "paranoid_getRecentTransactions",
        json!([2, 1, null]),
    )
    .await;
    assert_eq!(second_page["total_pages"], 4);
    assert_eq!(second_page["transactions"][0]["height"], 43);

    server.stop().unwrap();
    server.stopped().await;
}
