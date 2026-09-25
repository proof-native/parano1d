// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

use super::*;
use crate::contracts::{
    Info, Instances, LibraryEntry, Outcome, Preview, Request, LIBRARY_LIMIT, TERMS_LIMIT,
};
const RECEIPT_LIMIT: usize = 2 * 1024 * 1024;

async fn read_file(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| e.to_string())?;
    let metadata = file.metadata().await.map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err("Contract artifact is not a regular file within its size limit.".into());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("Contract artifact exceeds its size limit.".into());
    }
    Ok(bytes)
}

impl Backend {
    async fn contract_rpc<T: DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T, String> {
        if self.is_mock() {
            return Err("Connect to a v2 node to use contracts.".into());
        }
        self.rpc_with_timeout(method, params, Duration::from_secs(600))
            .await
            .map_err(contract_error)
    }

    async fn contract_instances(&self, info: Info, cursor: u32) -> Result<Outcome, String> {
        if !info.has_program_details() {
            return Err("The node did not provide the contract program. Update the node and reload the terms before funding.".into());
        }
        let instances: Instances = self
            .contract_rpc("getObjectInstances", json!([info.opening_hex, cursor, 32]))
            .await?;
        let mut known = self.contract_library().await?;
        if !instances.slots.is_empty() {
            if let Some(entry) = known.iter_mut().find(|entry| {
                entry
                    .candidate
                    .as_ref()
                    .is_some_and(|next| next.address == info.address)
            }) {
                let old: Instances = self
                    .contract_rpc("getObjectInstances", json!([entry.info.opening_hex, 0, 1]))
                    .await?;
                if !old.slots.is_empty() {
                    // Another deposit still uses these terms. Preserve it as a
                    // separate saved balance when following this successor.
                    entry.candidate = None;
                    self.save_contract_library(&known).await?;
                }
            }
        }
        let library = self
            .remember_contract(&info, None, !instances.slots.is_empty())
            .await?;
        Ok(Outcome::Loaded(info, instances, library))
    }

    pub async fn contract_operation(&self, request: Request) -> Result<Outcome, String> {
        match request {
            Request::Home => Ok(Outcome::Home(
                self.contract_rpc("getContractProtocol", json!([])).await?,
                self.contract_library().await?,
            )),
            Request::LoadSaved(index) => {
                let library = self.contract_library().await?;
                let entry = library
                    .get(index)
                    .ok_or("Saved contract list changed. Reload it.")?;
                // Stored descriptions are presentation only. Always decode and
                // validate the retained opening again through the daemon.
                let info = self
                    .contract_rpc("walletWatchObject", json!([entry.info.opening_hex]))
                    .await?;
                self.contract_instances(info, 0).await
            }
            Request::Rename(info, name) => Ok(Outcome::Library(
                self.remember_contract(&info, Some(name), false).await?,
            )),
            Request::Forget(index) => {
                let mut library = self.contract_library().await?;
                if index >= library.len() {
                    return Err("Saved contract list changed. Reload it.".into());
                }
                library.remove(index);
                self.save_contract_library(&library).await?;
                Ok(Outcome::Library(library))
            }
            Request::Preview { info, payload } => {
                let preview: Preview = self
                    .contract_rpc("previewObjectCall", json!([payload]))
                    .await?;
                Ok(Outcome::Previewed {
                    info,
                    payload,
                    preview,
                })
            }
            Request::Create(definition) => {
                let info: Info = self
                    .contract_rpc("createObject", json!([definition]))
                    .await?;
                let info = self
                    .contract_rpc("walletWatchObject", json!([info.opening_hex]))
                    .await?;
                self.contract_instances(info, 0).await
            }
            Request::Import => {
                let Some(file) = rfd::AsyncFileDialog::new()
                    .add_filter("Contract terms", &["json"])
                    .pick_file()
                    .await
                else {
                    return Ok(Outcome::Cancelled);
                };
                let value: Value =
                    serde_json::from_slice(&read_file(file.path(), TERMS_LIMIT).await?)
                        .map_err(|e| e.to_string())?;
                let opening = value["opening_hex"]
                    .as_str()
                    .or_else(|| value["successor"]["opening_hex"].as_str())
                    .ok_or("This file has no current contract terms.")?;
                // Ignore all file-supplied labels and authority descriptions.
                // The node decodes the opening and returns its actual policy.
                let info = self
                    .contract_rpc("walletWatchObject", json!([opening]))
                    .await?;
                self.contract_instances(info, 0).await
            }
            Request::Restore(address) => {
                let info = self
                    .contract_rpc("walletGetObjectOpening", json!([address]))
                    .await?;
                self.contract_instances(info, 0).await
            }
            Request::Refresh(info, cursor) => self.contract_instances(info, cursor).await,
            Request::Save(info) => {
                let Some(file) = rfd::AsyncFileDialog::new()
                    .set_file_name("contract.json")
                    .add_filter("Contract terms", &["json"])
                    .save_file()
                    .await
                else {
                    return Ok(Outcome::Cancelled);
                };
                file.write(&serde_json::to_vec_pretty(&info).map_err(|e| e.to_string())?)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(Outcome::Notice(
                    "Public contract terms saved. Share this file with the other participant."
                        .into(),
                ))
            }
            Request::Fund {
                info,
                amount,
                fee,
                sender,
            } => {
                let active: Value = self.contract_rpc("walletActiveAddress", json!([])).await?;
                if active["address"] != sender {
                    return Err("Active address changed. Review the transaction again.".into());
                }
                let result: Value = self
                    .contract_rpc(
                        "walletFundObject",
                        json!([info.opening_hex, amount, fee, sender]),
                    )
                    .await?;
                Ok(Outcome::Submitted {
                    txid: txid(&result)?,
                    old_opening: None,
                    successor: None,
                })
            }
            Request::Call {
                info,
                payload,
                authority,
                recovery,
                preview,
            } => {
                // Persist the possible successor before signing. Submission can
                // succeed even if the RPC connection later disappears.
                self.remember_contract(&info, None, true).await?;
                self.remember_successor(&info, preview.successor.clone())
                    .await?;
                let status: Value = self
                    .contract_rpc(
                        "getObjectStatus",
                        json!([info.opening_hex, payload["slot_index"]]),
                    )
                    .await?;
                let next_height = status["next_call_height"]
                    .as_u64()
                    .ok_or("Invalid contract status.")?;
                if status["active_authority"] != authority
                    || (next_height >= info.deadline_height) != recovery
                {
                    return Err("Contract authority or deadline branch changed. Review the transaction again.".into());
                }
                let result: Value = self
                    .contract_rpc("walletCallObject", json!([payload]))
                    .await?;
                let successor = if result["successor"].is_null() {
                    None
                } else {
                    Some(
                        serde_json::from_value(result["successor"].clone())
                            .map_err(|e| e.to_string())?,
                    )
                };
                Ok(Outcome::Submitted {
                    txid: txid(&result["transaction"])?,
                    old_opening: Some(info.opening_hex),
                    successor,
                })
            }
            Request::ExportReceipt { opening, txid } => {
                let encoded: String = self
                    .contract_rpc("exportObjectReceipt", json!([opening, txid]))
                    .await?;
                let checked: Value = self
                    .contract_rpc("verifyObjectReceipt", json!([encoded]))
                    .await?;
                if checked["valid"] != true || checked["txid"] != txid {
                    return Err("Receipt verification did not match the requested call.".into());
                }
                let bytes = hex::decode(&encoded).map_err(|e| e.to_string())?;
                if bytes.len() > RECEIPT_LIMIT {
                    return Err("Receipt exceeds its file limit.".into());
                }
                let Some(file) = rfd::AsyncFileDialog::new()
                    .set_file_name(format!("{txid}.receipt"))
                    .add_filter("Contract receipt", &["receipt"])
                    .save_file()
                    .await
                else {
                    return Ok(Outcome::Cancelled);
                };
                file.write(&bytes).await.map_err(|e| e.to_string())?;
                Ok(Outcome::Notice(format!(
                    "Verified receipt saved for transaction {txid}."
                )))
            }
            Request::VerifyReceipt => {
                let Some(file) = rfd::AsyncFileDialog::new()
                    .add_filter("Contract receipt", &["receipt"])
                    .pick_file()
                    .await
                else {
                    return Ok(Outcome::Cancelled);
                };
                let bytes = read_file(file.path(), RECEIPT_LIMIT).await?;
                let checked: Value = self
                    .contract_rpc("verifyObjectReceipt", json!([hex::encode(bytes)]))
                    .await?;
                if checked["valid"] != true {
                    return Err("Contract receipt did not verify.".into());
                }
                let notice = format!(
                    "Verified on this node's selected chain: transaction {} in block {}. This proves the recorded call; refresh current balances to check whether its successor remains spendable.",
                    txid(&checked)?, checked["height"]);
                if !checked["successor"].is_null() {
                    let successor: Info = serde_json::from_value(checked["successor"].clone())
                        .map_err(|e| e.to_string())?;
                    let successor = self
                        .contract_rpc("walletWatchObject", json!([successor.opening_hex]))
                        .await?;
                    let loaded = self.contract_instances(successor, 0).await?;
                    Ok(Outcome::WithNotice(Box::new(loaded), notice))
                } else {
                    Ok(Outcome::Notice(notice))
                }
            }
        }
    }
}

fn contract_error(error: String) -> String {
    match error.strip_suffix(" (-32000)").unwrap_or(&error) {
        "contract: Policy" => "This contract does not allow that action at the next block.".into(),
        "contract: FeeLimit" => "The network fee exceeds this contract's limit.".into(),
        "contract: ReserveLimit" => {
            "This payment would leave less than the contract's minimum reserve.".into()
        }
        "contract: PayoutLimit" => "Payment exceeds the per-call limit.".into(),
        "contract: Recipient" => "The recipient differs from the contract terms.".into(),
        "active wallet address is not the contract authority at this height" => {
            "The active address cannot authorize this contract at the next block.".into()
        }
        "opening or incarnation does not match the current contract State" => {
            "The selected balance has changed. Refresh and review the transaction again.".into()
        }
        "active wallet address changed; review the funding again"
        | "active wallet address changed; review the call again" => {
            "Active address changed. Review the transaction again.".into()
        }
        "inclusion height changed; preview and authorize the call again"
        | "call body changed; preview and authorize the call again" => {
            "Call details changed. Preview and review the transaction again.".into()
        }
        "contract: IntegerProgram(Assertion)" => {
            "The call does not satisfy the program conditions.".into()
        }
        "contract deadline branch changed; review the call again" => {
            "Contract authority or deadline branch changed. Review the transaction again.".into()
        }
        _ => error,
    }
}

fn txid(value: &Value) -> Result<String, String> {
    value["txid"]
        .as_str()
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(str::to_owned)
        .ok_or_else(|| "Node returned an invalid transaction ID.".into())
}

impl Backend {
    async fn contract_library(&self) -> Result<Vec<LibraryEntry>, String> {
        let path = self
            .config_snapshot()?
            .data_dir
            .join("wallet.contracts.json");
        if !tokio::fs::try_exists(&path)
            .await
            .map_err(|e| e.to_string())?
        {
            return Ok(Vec::new());
        }
        let entries: Vec<LibraryEntry> =
            serde_json::from_slice(&read_file(&path, LIBRARY_LIMIT * TERMS_LIMIT).await?)
                .map_err(|e| format!("Contract library: {e}"))?;
        if entries.len() > LIBRARY_LIMIT
            || entries.iter().any(|entry| {
                entry.name.chars().count() > 64
                    || entry.name.chars().any(char::is_control)
                    || !entry.info.has_program_details()
                    || entry.info.opening_hex.len() > 2 * 1024
                    || entry.candidate.as_ref().is_some_and(|candidate| {
                        !candidate.has_program_details() || candidate.opening_hex.len() > 2 * 1024
                    })
            })
        {
            return Err("Contract library exceeds its limits or has unsupported terms.".into());
        }
        Ok(entries)
    }

    async fn save_contract_library(&self, library: &[LibraryEntry]) -> Result<(), String> {
        let path = self
            .config_snapshot()?
            .data_dir
            .join("wallet.contracts.json");
        let bytes = serde_json::to_vec_pretty(library).map_err(|e| e.to_string())?;
        if library.len() > LIBRARY_LIMIT || bytes.len() > LIBRARY_LIMIT * TERMS_LIMIT {
            return Err(
                "Contract library is full. Export older terms before removing them from the list."
                    .into(),
            );
        }
        tokio::task::spawn_blocking(move || {
            persist_owner_only_atomically(&path, &bytes, "contract library")
        })
        .await
        .map_err(|e| e.to_string())?
    }

    async fn remember_successor(&self, info: &Info, successor: Option<Info>) -> Result<(), String> {
        let mut library = self.contract_library().await?;
        let entry = library
            .iter_mut()
            .find(|entry| entry.info.address == info.address)
            .ok_or("Saved contract list changed. Reload it.")?;
        entry.candidate = successor;
        self.save_contract_library(&library).await
    }

    async fn remember_contract(
        &self,
        info: &Info,
        name: Option<String>,
        confirmed: bool,
    ) -> Result<Vec<LibraryEntry>, String> {
        if !info.has_program_details() {
            return Err("Contract program details are incomplete.".into());
        }
        if name
            .as_ref()
            .is_some_and(|name| name.chars().count() > 64 || name.chars().any(char::is_control))
        {
            return Err(
                "Contract name must be at most 64 characters without control characters.".into(),
            );
        }
        let mut library = self.contract_library().await?;
        if let Some(entry) = library
            .iter_mut()
            .find(|entry| entry.info.address == info.address)
        {
            if entry.info.opening_hex != info.opening_hex {
                return Err("Saved contract opening changed.".into());
            }
            entry.info = info.clone();
            if let Some(name) = name {
                entry.name = name;
            }
        } else if let Some(entry) = library.iter_mut().find(|entry| {
            entry
                .candidate
                .as_ref()
                .is_some_and(|candidate| candidate.address == info.address)
        }) {
            if entry.candidate.as_ref().unwrap().opening_hex != info.opening_hex {
                return Err("Saved contract opening changed.".into());
            }
            // A state query, never submission alone, advances the saved entry.
            if confirmed {
                entry.info = info.clone();
                entry.candidate = None;
            }
            if let Some(name) = name {
                entry.name = name;
            }
        } else {
            library.push(LibraryEntry {
                name: name.unwrap_or_default(),
                info: info.clone(),
                candidate: None,
            });
        }
        self.save_contract_library(&library).await?;
        Ok(library)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backend(directory: &Path) -> Backend {
        Backend {
            inner: Arc::new(BackendInner {
                config: Mutex::new(BackendConfig {
                    rpc_url: DEFAULT_RPC_URL.into(),
                    rpc_listen: DEFAULT_RPC_LISTEN.into(),
                    p2p_listen: DEFAULT_P2P_LISTEN.into(),
                    data_dir: directory.into(),
                    node_binary: PathBuf::from("parano1d"),
                    seeds: Vec::new(),
                    log_level: LogLevel::Info,
                    language: None,
                    address_labels: BTreeMap::new(),
                    settings_path: directory.join("gui-settings.json"),
                    mock: false,
                }),
                client: Client::new(),
                next_request_id: AtomicU64::new(1),
                supervisor: Mutex::new(SupervisorState {
                    child: None,
                    owned: false,
                    desired_mode: NodeMode::Node,
                    selected_threads: 1,
                    genesis: false,
                }),
                system: Mutex::new(System::new()),
                wallet_utxo_cache: tokio::sync::Mutex::new(None),
            }),
        }
    }

    #[tokio::test]
    async fn saved_terms_survive_restart_and_pending_calls_do_not_replace_them() {
        let directory = tempfile::tempdir().unwrap();
        let first = backend(directory.path());
        let info = crate::contracts::tests::info();
        first
            .remember_contract(&info, Some("Household".into()), false)
            .await
            .unwrap();
        let mut successor = info.clone();
        successor.address = "next".into();
        successor.state[0] = "9007199254740993".into();
        successor.opening_hex = "bb".repeat(699);
        first
            .remember_successor(&info, Some(successor.clone()))
            .await
            .unwrap();
        drop(first);
        let reopened = backend(directory.path());
        let library = reopened.contract_library().await.unwrap();
        assert_eq!(library.len(), 1);
        assert_eq!(library[0].info.address, info.address);
        assert_eq!(
            library[0].candidate.as_ref().unwrap().state[0],
            "9007199254740993"
        );
        assert_eq!(library[0].name, "Household");
        reopened
            .remember_contract(&successor, None, false)
            .await
            .unwrap();
        assert_eq!(
            reopened.contract_library().await.unwrap()[0].info.address,
            info.address
        );
        reopened
            .remember_contract(&successor, None, true)
            .await
            .unwrap();
        let library = reopened.contract_library().await.unwrap();
        assert_eq!(library.len(), 1);
        assert_eq!(library[0].info.address, "next");
        assert_eq!(library[0].name, "Household");
        assert!(library[0].candidate.is_none());
        assert!(reopened
            .remember_contract(&successor, Some("bad\nname".into()), false)
            .await
            .is_err());
    }
}
