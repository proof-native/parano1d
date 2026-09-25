// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Explicit local qualification driver. It exercises the GUI backend with real
//! RPC and files; the Python scenario owns the isolated nodes and block production.

use super::*;

#[tokio::test]
#[ignore = "requires scripts/live_v2_contract_wallet_scenario.py and isolated nodes"]
async fn file_workflow_on_isolated_nodes() {
    let case_path = PathBuf::from(std::env::var("NOID_CONTRACT_WORKFLOW_CASE").unwrap());
    let case: Value = serde_json::from_slice(&std::fs::read(&case_path).unwrap()).unwrap();
    let data = Path::new(case["data_dir"].as_str().unwrap());
    std::fs::create_dir_all(data).unwrap();
    let backend = super::super::tests::backend(data);
    let url = case["rpc_url"].as_str().unwrap();
    assert!(url.starts_with("http://127.0.0.1:"));
    backend.inner.config.lock().unwrap().rpc_url = url.into();
    let protocol: Value = backend
        .contract_rpc("getContractProtocol", json!([]))
        .await
        .unwrap();
    assert_eq!(
        protocol["activation_height"], 10,
        "only the isolated fork schedule is allowed"
    );
    assert_eq!(protocol["active_at_next_block"], true);
    let result = match case["action"].as_str().unwrap() {
        "create_and_fund" => {
            let sender: Value = backend
                .contract_rpc("walletActiveAddress", json!([]))
                .await
                .unwrap();
            let review = backend
                .contract_operation(Request::CreateAndFund {
                    definition: case["definition"].clone(),
                    creation: Creation {
                        name: case["name"].as_str().unwrap().into(),
                        kind: crate::contracts::Kind::Custom,
                    },
                    amount: case["amount"].as_u64().unwrap(),
                    fee: 0,
                    sender: sender["address"].as_str().unwrap().into(),
                })
                .await
                .unwrap();
            let Outcome::FundingReview {
                info,
                amount,
                fee,
                sender,
                creation,
            } = review
            else {
                panic!("missing fee review")
            };
            let result = backend
                .contract_operation(Request::Fund {
                    info: info.clone(),
                    amount,
                    fee,
                    sender,
                    creation,
                })
                .await
                .unwrap();
            let Outcome::Submitted { txid, .. } = result else {
                panic!("funding was not recorded")
            };
            json!({"info": info, "txid": txid, "quoted_fee": fee})
        }
        "import" => {
            let file = backend
                .read_contract_file(Path::new(case["path"].as_str().unwrap()))
                .await
                .unwrap();
            backend.accept_contract_file(file).await.unwrap();
            json!({"library": backend.contract_library().await.unwrap()})
        }
        action => {
            let info: Info = backend
                .contract_rpc("walletWatchObject", json!([case["opening_hex"]]))
                .await
                .unwrap();
            match action {
                "share" => {
                    let file = backend.contract_share_file(&info).await.unwrap();
                    std::fs::write(
                        case["path"].as_str().unwrap(),
                        serde_json::to_vec_pretty(&file).unwrap(),
                    )
                    .unwrap();
                    json!({"with_receipt": file.proof.is_some()})
                }
                "activity" => {
                    let operations = backend.contract_activity(&info).await.unwrap();
                    let entries: Vec<Value> = operations
                        .into_iter()
                        .map(|op| {
                            let mut value = serde_json::to_value(&op).unwrap();
                            value["canonical"] = json!(op.canonical);
                            value["receipt_available"] = json!(op.receipt_available);
                            value
                        })
                        .collect();
                    json!({"operations": entries, "library": backend.contract_library().await.unwrap()})
                }
                "export_receipt" => {
                    let operations = backend.contract_activity(&info).await.unwrap();
                    let operation = operations
                        .iter()
                        .find(|op| op.txid == case["txid"].as_str().unwrap())
                        .unwrap();
                    let (name, bytes) = backend.operation_receipt_file(operation).await.unwrap();
                    std::fs::write(case["path"].as_str().unwrap(), &bytes).unwrap();
                    json!({"file_name": name, "bytes": bytes.len()})
                }
                "call" => {
                    let instances: Instances = backend
                        .contract_rpc("getObjectInstances", json!([info.opening_hex, 0, 1]))
                        .await
                        .unwrap();
                    let slot = &instances.slots[0];
                    let sender: Value = backend
                        .contract_rpc("walletActiveAddress", json!([]))
                        .await
                        .unwrap();
                    let mut payload = json!({"opening_hex": info.opening_hex, "slot_index": slot.slot_index,
                        "creation_id": slot.creation_id, "terminal": case["terminal"],
                        "payout": case["payout"], "fee_micronoid":0, "expected_authority":sender["address"]});
                    let preview: Preview = backend
                        .contract_rpc("previewObjectCall", json!([payload]))
                        .await
                        .unwrap();
                    payload["expected_txid"] = json!(preview.txid);
                    payload["expected_call_height"] = json!(preview.call_height);
                    payload["expected_recovery"] = json!(preview.recovery);
                    let result = backend
                        .contract_operation(Request::Call {
                            info,
                            payload,
                            authority: preview.authority.clone(),
                            recovery: preview.recovery,
                            preview,
                        })
                        .await
                        .unwrap();
                    let Outcome::Submitted {
                        txid, successor, ..
                    } = result
                    else {
                        panic!("missing submission")
                    };
                    json!({"txid":txid, "successor":successor})
                }
                other => panic!("unknown live GUI action: {other}"),
            }
        }
    };
    std::fs::write(
        case["result_path"].as_str().unwrap(),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
}
