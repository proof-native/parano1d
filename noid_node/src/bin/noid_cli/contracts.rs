// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

use super::*;
use serde_json::json;
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Subcommand)]
pub(super) enum ContractCommand {
    /// Save a received public opening and retain receipts for future calls.
    Watch { object: PathBuf },
    /// Payee collects before expiry; your wallet recovers afterwards.
    Payment {
        payee: String,
        expiry_height: u64,
        #[arg(long)]
        max_fee: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Lock a balance until a block height, including against your own key.
    Vault {
        unlock_height: u64,
        #[arg(long)]
        max_fee: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Give a spending key capped payments and keep a separate recovery key.
    Allowance {
        spending_key: String,
        recover_at: u64,
        #[arg(long)]
        max_payment: String,
        #[arg(long)]
        reserve: String,
        #[arg(long)]
        max_fee: String,
        #[arg(long)]
        payee: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Enforce a total debit budget, including fees, across each block period.
    Budget {
        spending_key: String,
        start_height: u64,
        period_blocks: u64,
        recover_at: u64,
        #[arg(long)]
        budget: String,
        #[arg(long)]
        max_payment: String,
        #[arg(long)]
        reserve: String,
        #[arg(long)]
        max_fee: String,
        #[arg(long)]
        payee: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Prepay fixed recurring charges without accumulating missed charges.
    Recurring {
        payee: String,
        first_due_height: u64,
        period_blocks: u64,
        recover_at: u64,
        #[arg(long)]
        payment: String,
        #[arg(long)]
        max_fee: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Unlock fixed tranches; missed tranches remain claimable one per call.
    Vesting {
        beneficiary: String,
        first_unlock_height: u64,
        period_blocks: u64,
        mature_at: u64,
        #[arg(long)]
        tranche: String,
        #[arg(long)]
        max_fee: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Show activation and the node's actual per-class contract limits.
    Protocol,
    /// Create a policy from an SDK definition JSON, including custom programs.
    Create {
        definition: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Fund an opening with an ordinary wallet payment (amounts in NOID).
    Fund {
        object: PathBuf,
        amount: String,
        #[arg(long, default_value = "0")]
        fee: String,
    },
    /// Compare the opening with one current State slot.
    Status { object: PathBuf, slot: u32 },
    /// Find funded incarnations directly in current State, even after pruning.
    Instances {
        object: PathBuf,
        #[arg(long, default_value_t = 0)]
        from_slot: u32,
        #[arg(long, default_value_t = 64)]
        limit: u32,
    },
    /// Continue or close one exact incarnation; retain its confirmation receipt.
    Call {
        object: PathBuf,
        slot: u32,
        creation_id: u64,
        #[arg(long)]
        close: bool,
        #[arg(long, requires = "pay_to")]
        pay: Option<String>,
        #[arg(long, requires = "pay")]
        pay_to: Option<String>,
        #[arg(long, default_value = "0")]
        fee: String,
        #[arg(long, required_unless_present = "preview")]
        out: Option<PathBuf>,
        /// Compute the exact call and its successor without signing or submitting.
        #[arg(long)]
        preview: bool,
        /// Refuse to sign if the freshly computed body differs from this review.
        #[arg(long)]
        expected_txid: Option<String>,
        /// Wait for confirmation and save a receipt; zero returns after submission.
        #[arg(long, default_value_t = 600)]
        wait_seconds: u64,
    },
    /// Export a retained receipt, including after block-body pruning.
    Receipt {
        object: PathBuf,
        txid: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Check a receipt against this node's selected chain and recursive proof.
    Verify { receipt: PathBuf },
    /// Restore a saved opening from the daemon's wallet directory.
    Restore {
        address: String,
        #[arg(long)]
        out: PathBuf,
    },
}

fn read_bounded(path: &Path, limit: usize) -> anyhow::Result<Vec<u8>> {
    let metadata = std::fs::metadata(path)?;
    anyhow::ensure!(
        metadata.is_file() && metadata.len() <= limit as u64,
        "input file exceeds its bound"
    );
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= limit, "input file grew beyond its bound");
    Ok(bytes)
}

fn read_opening(path: &Path) -> anyhow::Result<String> {
    let value: Value = serde_json::from_slice(&read_bounded(path, 16 * 1024)?)?;
    value["opening_hex"]
        .as_str()
        .or_else(|| value["successor"]["opening_hex"].as_str())
        .map(str::to_owned)
        .context("file has no current opening (a closed contract has no successor)")
}

fn new_output(path: &Path) -> anyhow::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .with_context(|| format!("create new artifact {}", path.display()))
}

fn save_json(mut file: File, value: &Value) -> anyhow::Result<()> {
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn sync_artifact_directory(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    File::open(
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?
    .sync_all()?;
    Ok(())
}

fn retain_review(path: &Path, request: &Value, reviewed: &Value) -> anyhow::Result<()> {
    // Persist before sending: a lost RPC response says nothing about whether
    // authorization and admission succeeded. Keep enough to query the exact
    // transaction and recover its candidate without signing a second call.
    save_json(
        new_output(path)?,
        &json!({
            "submission_status":"unknown",
            "transaction":{"txid":reviewed["txid"]},
            "successor":reviewed["successor"],
            "request":request,
            "preview":reviewed,
        }),
    )?;
    sync_artifact_directory(path)
}

fn replace_call_artifact(path: &Path, value: &Value) -> anyhow::Result<()> {
    let temporary = path.with_extension(format!("{:032x}.partial", rand::random::<u128>()));
    // Preserve the pre-submission review until the entire response is durable.
    // Neither a failed write nor interruption can leave a truncated review.
    let result = (|| {
        save_json(new_output(&temporary)?, value)?;
        std::fs::rename(&temporary, path)?;
        sync_artifact_directory(path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

async fn owner(ctx: &Ctx<'_>) -> anyhow::Result<String> {
    rpc(ctx, "walletActiveAddress", &[]).await?["address"]
        .as_str()
        .map(str::to_owned)
        .context("wallet has no active address")
}

async fn create(ctx: &Ctx<'_>, definition: Value, out: &Path) -> anyhow::Result<()> {
    let file = new_output(out)?;
    let info = rpc(ctx, "createObject", &[definition]).await?;
    save_json(file, &info)?;
    if ctx.json {
        print_json(&info)
    } else {
        println!(
            "Contract: {}\nOpening saved: {}",
            info["address"].as_str().unwrap_or(""),
            out.display()
        );
        Ok(())
    }
}

pub(super) async fn run(ctx: &Ctx<'_>, command: &ContractCommand) -> anyhow::Result<()> {
    match command {
        ContractCommand::Watch { object } => {
            let info = rpc(ctx, "walletWatchObject", &[json!(read_opening(object)?)]).await?;
            if ctx.json { print_json(&info) } else { println!("Watching contract {}", info["address"].as_str().unwrap_or("")); Ok(()) }
        }
        ContractCommand::Instances { object, from_slot, limit } => {
            let result = rpc(ctx, "getObjectInstances", &[json!(read_opening(object)?), json!(from_slot), json!(limit)]).await?;
            if ctx.json { print_json(&result) } else {
                for slot in result["slots"].as_array().context("contract slots missing")? {
                    println!("Slot {}  incarnation {}  {} NOID", slot["slot_index"], slot["creation_id"], noid_str(slot["value"].as_u64().unwrap_or(0)));
                }
                if !result["next_slot"].is_null() { println!("Next page: --from-slot {}", result["next_slot"]); }
                Ok(())
            }
        }
        ContractCommand::Payment { payee, expiry_height, max_fee, out } => create(ctx, json!({
            "kind":"refundable_payment", "payer":owner(ctx).await?, "payee":payee,
            "expiry_height":expiry_height, "max_fee_micronoid":parse_noid_amount(max_fee)?,
        }), out).await,
        ContractCommand::Vault { unlock_height, max_fee, out } => create(ctx, json!({
            "kind":"timelocked_vault", "owner":owner(ctx).await?, "unlock_height":unlock_height,
            "max_fee_micronoid":parse_noid_amount(max_fee)?,
        }), out).await,
        ContractCommand::Allowance { spending_key, recover_at, max_payment, reserve, max_fee, payee, out } => create(ctx, json!({
            "kind":"allowance_wallet", "spending_key":spending_key, "recovery_key":owner(ctx).await?,
            "payout_recipient":payee, "recover_at":recover_at, "max_fee_micronoid":parse_noid_amount(max_fee)?,
            "max_payout_micronoid":parse_noid_amount(max_payment)?, "min_retained_micronoid":parse_noid_amount(reserve)?,
        }), out).await,
        ContractCommand::Budget { spending_key, start_height, period_blocks, recover_at,
            budget, max_payment, reserve, max_fee, payee, out } => create(ctx, json!({
                "kind":"period_budget_wallet", "spending_key":spending_key,
                "recovery_key":owner(ctx).await?, "payout_recipient":payee,
                "start_height":start_height, "period_blocks":period_blocks,
                "budget_micronoid":parse_noid_amount(budget)?, "recover_at":recover_at,
                "max_fee_micronoid":parse_noid_amount(max_fee)?,
                "max_payout_micronoid":parse_noid_amount(max_payment)?,
                "min_retained_micronoid":parse_noid_amount(reserve)?,
            }), out).await,
        ContractCommand::Recurring { payee, first_due_height, period_blocks, recover_at,
            payment, max_fee, out } => create(ctx, json!({
                "kind":"recurring_payment", "payer":owner(ctx).await?, "payee":payee,
                "first_due_height":first_due_height, "period_blocks":period_blocks,
                "payment_micronoid":parse_noid_amount(payment)?, "recover_at":recover_at,
                "max_fee_micronoid":parse_noid_amount(max_fee)?,
            }), out).await,
        ContractCommand::Vesting { beneficiary, first_unlock_height, period_blocks,
            mature_at, tranche, max_fee, out } => create(ctx, json!({
                "kind":"tranche_vesting", "beneficiary":beneficiary,
                "first_unlock_height":first_unlock_height, "period_blocks":period_blocks,
                "tranche_micronoid":parse_noid_amount(tranche)?, "mature_at":mature_at,
                "max_fee_micronoid":parse_noid_amount(max_fee)?,
            }), out).await,
        ContractCommand::Protocol => print_json(&rpc(ctx, "getContractProtocol", &[]).await?),
        ContractCommand::Create { definition, out } => create(ctx, serde_json::from_slice(&read_bounded(definition, 16 * 1024)?)?, out).await,
        ContractCommand::Fund { object, amount, fee } => {
            let result = rpc(ctx, "walletFundObject", &[json!(read_opening(object)?), json!(parse_noid_amount(amount)?), json!(parse_noid_amount(fee)?)]).await?;
            if ctx.json { print_json(&result) } else { println!("Funding submitted: {}", result["txid"].as_str().unwrap_or("")); Ok(()) }
        }
        ContractCommand::Status { object, slot } => {
            let result = rpc(ctx, "getObjectStatus", &[json!(read_opening(object)?), json!(slot)]).await?;
            if ctx.json { print_json(&result) } else {
                println!("Opening matches State: {}\nBalance: {} NOID\nIncarnation: {}\nNext-call authority: {}",
                    result["matches_opening"], noid_str(result["slot"]["value"].as_u64().unwrap_or(0)), result["slot"]["creation_id"], result["active_authority"].as_str().unwrap_or("")); Ok(())
            }
        }
        ContractCommand::Call { object, slot, creation_id, close, pay, pay_to, fee, out, preview, expected_txid, wait_seconds } => {
            anyhow::ensure!(!*close || pay.is_none(), "closing has no second payment");
            let opening = read_opening(object)?;
            let payout = match (pay, pay_to) {
                (Some(amount), Some(address)) => json!({"address":address,"amount_micronoid":parse_noid_amount(amount)?}),
                _ => Value::Null,
            };
            let mut request = json!({
                "opening_hex":opening, "slot_index":slot, "creation_id":creation_id, "terminal":close,
                "payout":payout, "fee_micronoid":parse_noid_amount(fee)?,
                "expected_authority":owner(ctx).await?, "expected_txid":expected_txid,
            });
            let reviewed = rpc(ctx, "previewObjectCall", &[request.clone()]).await?;
            if *preview { return print_json(&reviewed); }
            bind_review(&mut request, &reviewed)?;
            let out = out.as_ref().context("--out is required when submitting a call")?;
            retain_review(out, &request, &reviewed)?;
            let mut result = rpc(ctx, "walletCallObject", &[request.clone()]).await
                .with_context(|| format!("Call outcome is not confirmed. Review and transaction ID are saved in {}; inspect this transaction before authorizing another call", out.display()))?;
            anyhow::ensure!(
                result["transaction"]["txid"] == reviewed["txid"]
                    && result["call_height"] == reviewed["call_height"]
                    && result["successor"] == reviewed["successor"],
                "call response differs from the saved review; inspect the reviewed transaction before authorizing again"
            );
            result["request"] = request;
            result["preview"] = reviewed;
            result["submission_status"] = json!("submitted");
            replace_call_artifact(out, &result)?;
            let txid = result["transaction"]["txid"].as_str().context("call transaction id missing")?;
            if *wait_seconds == 0 {
                if ctx.json { return print_json(&result); }
                println!("Call submitted: {txid}\nCandidate successor saved: {}. Check confirmation before using it.", out.display());
                return Ok(());
            }
            let until = std::time::Instant::now().checked_add(std::time::Duration::from_secs(*wait_seconds))
                .context("wait duration is too large")?;
            loop {
                if !rpc(ctx, "getTx", &[json!(txid)]).await?.is_null() { break; }
                if rpc(ctx, "getMempoolEntry", &[json!(txid)]).await?.is_null() {
                    // Confirmation can race with the first lookup and eviction.
                    if !rpc(ctx, "getTx", &[json!(txid)]).await?.is_null() { break; }
                    bail!("call is no longer pending on this node; inspect its transaction ID and current contract balance before authorizing again. Height-dependent calls may require a fresh preview. Saved successor is only a candidate until confirmed");
                }
                anyhow::ensure!(std::time::Instant::now() < until, "call remains pending; artifact is saved; export its receipt after confirmation");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            let receipt = rpc(ctx, "exportObjectReceipt", &[json!(opening), json!(txid)]).await?;
            let encoded = receipt.as_str().context("receipt encoding missing")?;
            let checked = rpc(ctx, "verifyObjectReceipt", &[json!(encoded)]).await?;
            anyhow::ensure!(checked["valid"] == true && checked["txid"] == txid, "contract receipt verification failed");
            let receipt_path = out.with_extension("receipt");
            let mut receipt_file = new_output(&receipt_path)?;
            receipt_file.write_all(&hex::decode(encoded)?)?;
            receipt_file.sync_all()?;
            if ctx.json { print_json(&json!({"call":result,"receipt":checked,"receipt_file":receipt_path})) } else {
                println!("Confirmed call: {txid}\nReceipt verified and saved: {}", receipt_path.display()); Ok(())
            }
        }
        ContractCommand::Receipt { object, txid, out } => {
            let mut file = new_output(out)?;
            let result = rpc(ctx, "exportObjectReceipt", &[json!(read_opening(object)?), json!(txid)]).await?;
            file.write_all(&hex::decode(result.as_str().context("receipt encoding missing")?)?)?; file.sync_all()?;
            if ctx.json { print_json(&json!({"receipt_file":out})) } else { println!("Receipt saved: {}", out.display()); Ok(()) }
        }
        ContractCommand::Verify { receipt } => {
            let bytes = read_bounded(receipt, noid_block::contract_receipt::MAX_RECEIPT_BYTES)?;
            let result = rpc(ctx, "verifyObjectReceipt", &[json!(hex::encode(bytes))]).await?;
            if ctx.json { print_json(&result) } else { println!("Verified on selected chain at height {}: {}", result["height"], result["txid"].as_str().unwrap_or("")); Ok(()) }
        }
        ContractCommand::Restore { address, out } => {
            let file = new_output(out)?;
            let result = rpc(ctx, "walletGetObjectOpening", &[json!(address)]).await?; save_json(file, &result)?;
            if ctx.json { print_json(&result) } else { println!("Opening restored: {}", out.display()); Ok(()) }
        }
    }
}

/// Sign exactly the body the node previewed. No silent retries after a changed
/// height, fee, incarnation or output reservation.
fn bind_review(request: &mut Value, reviewed: &Value) -> anyhow::Result<()> {
    let txid = reviewed["txid"]
        .as_str()
        .context("preview transaction ID missing")?;
    anyhow::ensure!(
        txid.len() == 64 && txid.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid preview transaction ID"
    );
    let height = reviewed["call_height"]
        .as_u64()
        .context("preview height missing")?;
    let authority = reviewed["authority"]
        .as_str()
        .context("preview authority missing")?;
    let recovery = reviewed["recovery"]
        .as_bool()
        .context("preview branch missing")?;
    anyhow::ensure!(
        request["expected_authority"] == authority,
        "active wallet cannot authorize the previewed call"
    );
    request["expected_txid"] = json!(txid);
    request["expected_call_height"] = json!(height);
    request["expected_recovery"] = json!(recovery);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_review_survives_a_lost_response_and_failed_response_write() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("call.json");
        let review = json!({"txid":"ab".repeat(32), "successor":{"opening_hex":"cafe"}});
        let request = json!({"expected_txid":review["txid"], "opening_hex":"beef"});
        retain_review(&path, &request, &review).unwrap();
        let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["submission_status"], "unknown");
        assert_eq!(saved["transaction"]["txid"], review["txid"]);
        assert_eq!(saved["request"], request);
        assert_eq!(read_opening(&path).unwrap(), "cafe");
        assert!(retain_review(&path, &request, &review).is_err());
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(&path).unwrap()).unwrap(),
            saved
        );
        let updated = json!({"submission_status":"submitted", "successor":review["successor"]});
        replace_call_artifact(&path, &updated).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(&path).unwrap()).unwrap(),
            updated
        );
        // A destination changed to a directory causes atomic replacement to
        // fail. It must not overwrite its contents or leave a temporary file.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("held"), b"keep").unwrap();
        assert!(replace_call_artifact(&path, &updated).is_err());
        assert_eq!(std::fs::read(path.join("held")).unwrap(), b"keep");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn call_signing_is_bound_to_the_complete_review_and_authority() {
        let reviewed = json!({"txid":"ab".repeat(32), "call_height":10, "authority":"alice", "recovery":false});
        let mut request = json!({"expected_authority":"alice"});
        bind_review(&mut request, &reviewed).unwrap();
        assert_eq!(request["expected_txid"], reviewed["txid"]);
        assert_eq!(request["expected_call_height"], 10);
        assert_eq!(request["expected_recovery"], false);
        request["expected_authority"] = json!("bob");
        assert!(bind_review(&mut request, &reviewed).is_err());
        request["expected_authority"] = json!("alice");
        let mut malformed = reviewed.clone();
        malformed["txid"] = json!("bad");
        assert!(bind_review(&mut request, &malformed).is_err());
    }
}
