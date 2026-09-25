// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.
//! parano1d-cli — Paranoid full node CLI client.
//!
//! Connects to a running `parano1d` daemon via JSON-RPC (no local keys, no crypto).
//! All operations happen inside the daemon; the CLI is a thin terminal UI.
//!
//! Quick start:
//!   parano1d-cli status            — node health at a glance
//!   parano1d-cli balance           — wallet balance
//!   parano1d-cli send <addr> 10.5  — send 10.5 NOID
//!   parano1d-cli help              — full command list

#![allow(clippy::format_in_format_args, clippy::print_literal)]

use anyhow::{bail, Context};
use clap::{Parser, Subcommand};
use noid_chain::consensus::params::RETAINED_BLOCK_SERVING_DEPTH;
use serde_json::Value;
use std::io::{self, IsTerminal, Write};

#[path = "noid_cli/contracts.rs"]
mod contracts;

// ---------------------------------------------------------------------------
// ANSI terminal colours (no external crate needed)
// ---------------------------------------------------------------------------

const RST: &str = "\x1b[0m"; // reset
const BOLD: &str = "\x1b[1m"; // bold
const DIM: &str = "\x1b[2m"; // dim
const RED: &str = "\x1b[31m"; // error
const GRN: &str = "\x1b[32m"; // success / positive
const YLW: &str = "\x1b[33m"; // warning
const CYN: &str = "\x1b[36m"; // label / key
const WHT: &str = "\x1b[97m"; // bright white value

/// Check whether stdout is a real terminal (disable colours when piped).
fn is_tty() -> bool {
    io::stdout().is_terminal()
        && std::env::var("TERM").is_ok_and(|t| t != "dumb")
        && std::env::var("NO_COLOR").is_err()
        && std::env::var("CI").is_err()
}

/// Return coloured string only when outputting to a terminal.
macro_rules! c {
    ($colour:expr, $text:expr) => {{
        if is_tty() {
            format!("{}{}{}", $colour, $text, RST)
        } else {
            $text.to_string()
        }
    }};
}

// ---------------------------------------------------------------------------
// Units
// ---------------------------------------------------------------------------

const MICRO_PER_NOID: u64 = 1_000_000;

/// Parse a human amount like "10.5" or "0.000001" as NOID → μNOID.
fn parse_noid_amount(s: &str) -> anyhow::Result<u64> {
    let (whole, fractional) = match s.split_once('.') {
        Some((whole, fractional)) if !fractional.contains('.') => (whole, fractional),
        Some(_) => bail!("invalid amount {s:?}: more than one decimal point"),
        None => (s, ""),
    };
    if whole.is_empty() && fractional.is_empty() {
        bail!("amount cannot be empty");
    }
    if whole.starts_with('-') || fractional.starts_with('-') {
        bail!("amount cannot be negative");
    }
    if fractional.len() > 6 {
        bail!("amount has more than 6 decimal places");
    }
    if (!whole.is_empty() && !whole.bytes().all(|byte| byte.is_ascii_digit()))
        || !fractional.bytes().all(|byte| byte.is_ascii_digit())
    {
        bail!("invalid amount {s:?}: expected a decimal number like 10.5 or 0.000001");
    }
    let whole = if whole.is_empty() {
        0
    } else {
        whole
            .parse::<u64>()
            .with_context(|| format!("amount {s:?} is out of range"))?
    };
    let fractional = if fractional.is_empty() {
        0
    } else {
        let digits = fractional
            .parse::<u64>()
            .with_context(|| format!("invalid fractional amount {s:?}"))?;
        digits
            .checked_mul(10u64.pow(6 - fractional.len() as u32))
            .ok_or_else(|| anyhow::anyhow!("amount {s:?} is out of range"))?
    };
    whole
        .checked_mul(MICRO_PER_NOID)
        .and_then(|value| value.checked_add(fractional))
        .ok_or_else(|| anyhow::anyhow!("amount {s:?} is out of range"))
}

fn noid_str(micronoid: u64) -> String {
    format!(
        "{}.{:06}",
        micronoid / MICRO_PER_NOID,
        micronoid % MICRO_PER_NOID
    )
}

fn fmt_hash(h: &str) -> &str {
    h.trim_start_matches("0x")
}

// ---------------------------------------------------------------------------
// CLI structure
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "parano1d-cli",
    about = "ParanO(1)d thin client — control a running parano1d daemon",
    version = env!("CARGO_PKG_VERSION"),
    long_about = "\
ParanO(1)d thin client. Connects to a running parano1d daemon via JSON-RPC.

QUICK START:
  parano1d-cli status              Node info (height, hash, slots)
  parano1d-cli balance             Wallet balance
  parano1d-cli send <addr> 10.5   Send 10.5 NOID to address
  parano1d-cli history             Transaction history
  parano1d-cli mempool             Pending transactions
  parano1d-cli help                All commands

AMOUNT FORMAT:
  Amounts are in NOID (e.g. 10.5, 0.000001).
  1 NOID = 1,000,000 μNOID — the CLI converts automatically.

DAEMON:
  The daemon must be running: parano1d --mode miner --data-dir ~/.parano1d/data",
)]
struct Cli {
    /// JSON-RPC endpoint of the running parano1d daemon.
    #[arg(
        long,
        short = 'r',
        default_value = "http://127.0.0.1:9601",
        env = "NOID_RPC",
        value_name = "URL",
        global = true
    )]
    rpc: String,

    /// Output raw JSON (for scripting / piping to jq).
    #[arg(long, short = 'j', global = true)]
    json: bool,

    /// Maximum seconds for one RPC, including wallet proof construction.
    #[arg(long, default_value_t = 60, global = true, value_parser = clap::value_parser!(u64).range(1..=3600))]
    rpc_timeout: u64,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create, fund, call and verify v2 contracts.
    Contract {
        #[command(subcommand)]
        action: contracts::ContractCommand,
    },
    // ---- Chain ----------------------------------------------------------------
    /// Node status: height, best hash, difficulty, active UTXOs.
    Status,

    /// Block hash at a given height.
    #[command(name = "block-hash", alias = "bh")]
    BlockHash {
        /// Block height.
        height: u64,
    },

    /// Decoded block header at a given height (all fields as structured data).
    #[command(name = "block-header", alias = "bhead")]
    BlockHeader {
        /// Block height.
        height: u64,
    },

    /// Full raw block at a given height (retained recent blocks only).
    #[command(alias = "blk")]
    Block {
        /// Block height to query.
        height: u64,
    },

    /// Raw 212-byte block header hex (for developers).
    Header {
        /// Block height to query.
        height: u64,
    },

    /// Current finalized HistoryStep terminal.
    HistoryStepTerminal,

    /// UTXO slot by index: value and owner.
    Slot {
        /// Slot index (0-based).
        index: u32,
    },

    /// All UTXOs owned by an address (bech32m).
    #[command(name = "utxos-of")]
    UtxosOf {
        /// Owner address (bech32m o1…).
        #[arg(value_name = "ADDRESS")]
        address: String,
    },

    /// Confirmed transaction info by hash.
    Tx {
        /// Transaction body hash (64-char hex).
        #[arg(value_name = "TX_HASH")]
        txhash: String,
    },

    /// UTXO state dimensions: capacity, fill %, size on disk, expansion headroom.
    State,

    /// Mining info: difficulty, block reward, HistoryStep height.
    Mining,

    /// Number of connected peers.
    Peers,

    /// Estimate minimum relay fee for live inputs/outputs.
    #[command(name = "estimate-fee")]
    EstimateFee {
        /// Number of outputs in the transaction (default: 2).
        #[arg(default_value_t = 2)]
        n_outputs: u32,
        /// Number of live inputs in the transaction (default: 1).
        #[arg(long, default_value_t = 1, value_name = "N")]
        inputs: u32,
    },

    /// Validate an address and show its canonical bech32m form.
    Validate {
        /// Address to validate (bech32m).
        #[arg(value_name = "ADDRESS")]
        address: String,
    },

    /// Current epoch anchor hash (needed by wallets to build transactions).
    #[command(alias = "anchor")]
    Epoch,

    /// Pending transactions in the mempool.
    Mempool,

    /// Single pending transaction by hash.
    #[command(name = "mempool-tx")]
    MempoolTx {
        /// Transaction body hash (64-char hex).
        #[arg(value_name = "TX_HASH")]
        txhash: String,
    },

    // ---- Wallet ---------------------------------------------------------------
    /// Show and manage wallet addresses.
    #[command(alias = "addr")]
    Address {
        /// Generate the next inactive address.
        #[arg(long)]
        new: bool,
        /// List locally generated addresses (the active one is marked).
        #[arg(long)]
        list: bool,
        /// Show address at a specific key index.
        #[arg(long, value_name = "INDEX")]
        index: Option<u32>,
        /// Switch the ACTIVE address to this key index. Sends spend from the
        /// active address only (one owner per transaction); change returns
        /// to it.
        #[arg(long = "use", value_name = "INDEX")]
        use_index: Option<u32>,
    },

    /// Confirmed wallet balance (NOID and μNOID).
    #[command(alias = "bal")]
    Balance,

    /// List all confirmed UTXOs with slot index, value, and height.
    #[command(alias = "ls")]
    Utxos,

    /// Send NOID to a recipient address.
    ///
    /// Amount is in NOID (e.g. 10.5 → 10,500,000 μNOID).
    /// Fee is auto-computed if not specified (recommended).
    ///
    /// Examples:
    ///   parano1d-cli send f784...b61e 10.5
    ///   parano1d-cli send f784...b61e 10.5 --fee 0.01
    Send {
        /// Recipient address (canonical bech32m o1...).
        #[arg(value_name = "ADDRESS")]
        to: String,
        /// Amount in NOID  (1 NOID = 1 000 000 μNOID).
        /// Examples: "50" = 50 NOID, "0.5" = 500 000 μNOID, "0.000001" = 1 μNOID (minimum).
        /// Tip: for programmatic use the RPC walletSend accepts raw μNOID directly.
        #[arg(value_name = "AMOUNT")]
        amount: String,
        /// Transaction fee in NOID. Omit for automatic fee calculation.
        #[arg(long, value_name = "FEE_NOID")]
        fee: Option<String>,
        /// Show the ordinary-transaction plan without submitting.
        #[arg(long)]
        dry_run: bool,
    },

    /// Transaction history: received and sent transactions.
    #[command(alias = "hist", alias = "txs")]
    History {
        /// Filter by a specific wallet address (bech32m).
        #[arg(long, value_name = "ADDRESS")]
        address: Option<String>,
        /// Show only the last N entries.
        #[arg(long, value_name = "N")]
        last: Option<usize>,
    },

    /// Reload the active address from the verified owner index.
    Scan,

    /// Export a Merkle payment receipt for a confirmed transaction.
    /// Redirect output to a file: parano1d-cli receipt <hash> > receipt.hex
    Receipt {
        /// Transaction hash (64-char hex).
        #[arg(value_name = "TX_HASH")]
        txhash: String,
    },

    /// Verify a Merkle payment receipt against the canonical chain.
    #[command(alias = "check")]
    Verify {
        /// Receipt bytes as hex string (from 'receipt' command).
        #[arg(value_name = "RECEIPT_HEX")]
        receipt: String,
    },

    // ---- Node control ---------------------------------------------------------
    /// Gracefully stop the Parano1d daemon.
    Stop,

    // ---- Mining (external miner API) ------------------------------------------
    /// Get a block template for an external Poseidon2b PoW miner.
    #[command(name = "block-template", alias = "template")]
    BlockTemplate {
        /// Coinbase address for this template (bech32m o1...). Defaults to wallet address.
        #[arg(long, value_name = "ADDRESS", default_value = "")]
        miner_addr: String,
    },

    /// Submit a nonce for a node-owned, single-use mining template.
    #[command(name = "submit-block", alias = "submit")]
    SubmitBlock {
        /// Opaque template identifier returned by `block-template`.
        #[arg(value_name = "TEMPLATE_ID")]
        template_id: String,
        /// Exactly 16 little-endian nonce bytes as 32 lowercase hex characters.
        #[arg(value_name = "NONCE_HEX")]
        nonce_hex: String,
    },
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    // Suppress broken-pipe panics: happen when stdout is piped to `head`, etc.
    // Without this, `parano1d-cli utxos | head -5` prints a Rust panic traceback.
    // The payload can be either &str or String depending on the panic site.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg_str = info.payload().downcast_ref::<&str>().copied().unwrap_or("");
        let msg_string = info
            .payload()
            .downcast_ref::<String>()
            .map(|s| s.as_str())
            .unwrap_or("");
        if msg_str.contains("Broken pipe") || msg_string.contains("Broken pipe") {
            std::process::exit(0);
        }
        default_hook(info);
    }));

    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        // Use {:#} to get the full error chain (context: cause: ...)
        // so print_error can detect "Node is not responding" in any layer.
        print_error(&format!("{e:#}"));
        std::process::exit(1);
    }

    // Flush stdout before exit (avoids broken-pipe on some platforms).
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("parano1d-cli/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(cli.rpc_timeout))
        .build()?;

    let ctx = Ctx {
        client: &client,
        rpc: &cli.rpc,
        json: cli.json,
    };

    match &cli.cmd {
        Command::Contract { action } => contracts::run(&ctx, action).await,
        Command::Status => cmd_status(&ctx).await,
        Command::BlockHash { height } => cmd_block_hash(&ctx, *height).await,
        Command::BlockHeader { height } => cmd_block_header(&ctx, *height).await,
        Command::Block { height } => cmd_block(&ctx, *height).await,
        Command::Header { height } => cmd_header(&ctx, *height).await,
        Command::HistoryStepTerminal => cmd_history_step_terminal(&ctx).await,
        Command::Slot { index } => cmd_slot(&ctx, *index).await,
        Command::UtxosOf { address } => cmd_utxos_of(&ctx, address).await,
        Command::Tx { txhash } => cmd_tx(&ctx, txhash).await,
        Command::State => cmd_state(&ctx).await,
        Command::Mining => cmd_mining(&ctx).await,
        Command::Peers => cmd_peers(&ctx).await,
        Command::EstimateFee { n_outputs, inputs } => {
            cmd_estimate_fee(&ctx, *inputs, *n_outputs).await
        }
        Command::Validate { address } => cmd_validate(&ctx, address).await,
        Command::Epoch => cmd_epoch(&ctx).await,
        Command::Mempool => cmd_mempool(&ctx).await,
        Command::MempoolTx { txhash } => cmd_mempool_tx(&ctx, txhash).await,
        Command::Address {
            new,
            list,
            index,
            use_index,
        } => cmd_address(&ctx, *new, *list, *index, *use_index).await,
        Command::Balance => cmd_balance(&ctx).await,
        Command::Utxos => cmd_utxos(&ctx).await,
        Command::Send {
            to,
            amount,
            fee,
            dry_run,
        } => cmd_send(&ctx, to, amount, fee.as_deref(), *dry_run).await,
        Command::History { address, last } => cmd_history(&ctx, address.as_deref(), *last).await,
        Command::Scan => cmd_scan(&ctx).await,
        Command::Receipt { txhash } => cmd_receipt(&ctx, txhash).await,
        Command::Verify { receipt } => cmd_verify(&ctx, receipt).await,
        Command::Stop => cmd_stop(&ctx).await,
        Command::BlockTemplate { miner_addr } => cmd_block_template(&ctx, miner_addr).await,
        Command::SubmitBlock {
            template_id,
            nonce_hex,
        } => cmd_submit_block(&ctx, template_id, nonce_hex).await,
    }
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

struct Ctx<'a> {
    client: &'a reqwest::Client,
    rpc: &'a str,
    json: bool,
}

impl<'a> Ctx<'a> {
    fn h<'h>(&self, hash: &'h str) -> &'h str {
        fmt_hash(hash)
    }
}

// ---------------------------------------------------------------------------
// Output helpers
// ---------------------------------------------------------------------------

fn print_error(msg: &str) {
    // Check for common connection errors
    let human = if msg.contains("Connection refused")
        || msg.contains("connection refused")
        || msg.contains("ConnectError")
        || msg.contains("Node is not responding")
    {
        "Node is not responding.\n\
             Is the parano1d daemon running?  Try: parano1d --mode miner\n\
             Default RPC: http://127.0.0.1:9601  (override with --rpc)"
            .to_string()
    } else if msg.contains("Insufficient") || msg.contains("insufficient") {
        // Extract amounts from error if possible
        msg.replace("InsufficientFunds", "Insufficient funds")
            .replace("_", " ")
    } else if msg.contains("Bad address")
        || msg.contains("bad address")
        || msg.contains("invalid address")
        || msg.contains("WrongHrp")
    {
        "Invalid address.\nUse a bech32m address (o1…).\nExample: parano1d-cli send o1q9gnyj0zwhqj9tm5sf… 10.5".to_string()
    } else {
        msg.to_string()
    };

    eprintln!("{} {}", c!(RED, "Error:"), human);
}

fn section(title: &str) {
    if is_tty() {
        println!("{}{}{}", BOLD, title, RST);
    } else {
        println!("{title}");
    }
}

fn kv(key: &str, val: &str) {
    if is_tty() {
        println!("  {}{:<18}{} {}{}{}", CYN, key, RST, WHT, val, RST);
    } else {
        println!("  {key:<18} {val}");
    }
}

fn kv2(key: &str, main: &str, sub: &str) {
    if is_tty() {
        println!(
            "  {}{:<18}{} {}{}{} {}{}{}",
            CYN, key, RST, WHT, main, RST, DIM, sub, RST
        );
    } else {
        println!("  {key:<18} {main}  {sub}");
    }
}

fn ok_msg(msg: &str) {
    println!("{} {}", c!(GRN, "✓"), msg);
}

fn warn_msg(msg: &str) {
    println!("{} {}", c!(YLW, "⚠"), msg);
}

fn separator(width: usize) {
    println!("  {}", c!(DIM, &"─".repeat(width)));
}

// ---------------------------------------------------------------------------
// Chain commands
// ---------------------------------------------------------------------------

async fn cmd_status(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    let info = rpc(ctx, "getChainInfo", &[])
        .await
        .context("getChainInfo")?;

    if ctx.json {
        return print_json(&info);
    }

    let height = info["height"].as_u64().unwrap_or(0);
    let best_hash = info["best_hash"].as_str().unwrap_or("?");
    let diff = info["difficulty_target"].as_str().unwrap_or("?");
    let slots = info["active_slot_count"].as_u64().unwrap_or(0);
    let log_slots = info["log_slots"].as_u64().unwrap_or(0);
    let capacity = 1u64 << log_slots.min(63);
    let fill_pct = slots.saturating_mul(100).checked_div(capacity).unwrap_or(0);

    let diff_bits = hex::decode(diff)
        .ok()
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .map(|target| noid_chain::consensus::target_leading_zero_bits(&target));

    section("Paranoid node status");
    kv("Height", &height.to_string());
    kv("Best hash", ctx.h(best_hash));
    kv2(
        "Difficulty",
        &diff_bits.map_or_else(|| "unknown".into(), |bits| format!("{bits} leading zeros")),
        &format!("(0x{})", &diff[..diff.len().min(16)]),
    );
    kv2(
        "Active UTXOs",
        &format!("{slots}"),
        &format!("({fill_pct}% of {capacity} slots, log={log_slots})"),
    );

    // Also fetch mempool size for quick overview
    if let Ok(mp) = rpc(ctx, "getMempoolSize", &[]).await {
        let n = mp.as_u64().unwrap_or(0);
        kv("Mempool", &format!("{n} pending tx(s)"));
    }

    Ok(())
}

async fn cmd_block(ctx: &Ctx<'_>, height: u64) -> anyhow::Result<()> {
    let result = rpc(ctx, "getBlock", &[height.into()])
        .await
        .context("getBlock")?;

    if ctx.json {
        return print_json(&result);
    }

    if result.is_null() {
        warn_msg(&format!(
            "Block {height} not available (the local complete-bundle reserve covers up to the last {RETAINED_BLOCK_SERVING_DEPTH} heights after warm-up)."
        ));
        return Ok(());
    }

    // The block is raw hex — show basic info
    let hex = result.as_str().unwrap_or("");
    section(&format!("Block #{height}"));
    kv(
        "Hex length",
        &format!("{} bytes ({} hex chars)", hex.len() / 2, hex.len()),
    );
    kv2(
        "Header (first 276B)",
        ctx.h(&hex[..hex.len().min(64)]),
        "(raw hex)",
    );
    println!();
    println!("  {} Use --json to get the full raw hex.", c!(DIM, "Tip:"));

    Ok(())
}

async fn cmd_header(ctx: &Ctx<'_>, height: u64) -> anyhow::Result<()> {
    let result = rpc(ctx, "getHeaderByHeight", &[height.into()])
        .await
        .context("getHeaderByHeight")?;

    if ctx.json {
        return print_json(&result);
    }

    if result.is_null() {
        warn_msg(&format!("No header found at height {height}."));
        return Ok(());
    }

    // For developers: print the raw hex
    let hex = result.as_str().unwrap_or("");
    section(&format!("Block header #{height}"));
    kv("Size", &format!("{} bytes (276)", hex.len() / 2));
    println!();
    // Print in 80-char rows for readability
    let hex_out = hex;
    for (i, chunk) in hex_out.as_bytes().chunks(80).enumerate() {
        let row = std::str::from_utf8(chunk).unwrap_or("");
        if i == 0 {
            println!("  {}", row);
        } else {
            println!("  {}{}{}", DIM, row, RST);
        }
    }

    Ok(())
}

async fn cmd_history_step_terminal(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    let result = rpc(ctx, "getHistoryStepTerminal", &[])
        .await
        .context("getHistoryStepTerminal")?;

    if ctx.json {
        return print_json(&result);
    }

    if result.is_null() {
        warn_msg("No HistoryStep terminal is available at the finalized boundary.");
        return Ok(());
    }

    let hex = result.as_str().unwrap_or("");
    let bytes = hex.len() / 2;
    let kb = bytes as f64 / 1024.0;

    // Simple fingerprint: first 16 + last 16 chars of the hex
    let terminal_fingerprint = if hex.len() >= 32 {
        format!("{}…{}", &hex[..8], &hex[hex.len() - 8..])
    } else {
        hex[..hex.len().min(16)].to_string()
    };

    section("HistoryStep terminal");
    kv2(
        "Size",
        &format!("{bytes} bytes ({kb:.1} KB)"),
        "(constant-size history envelope)",
    );
    kv("Fingerprint", &terminal_fingerprint);
    println!();
    println!(
        "  {} The terminal binds the selected chain boundary for O(1)-history verification.",
        c!(DIM, "Note:")
    );

    Ok(())
}

async fn cmd_slot(ctx: &Ctx<'_>, index: u32) -> anyhow::Result<()> {
    let result = rpc(ctx, "getSlot", &[index.into()])
        .await
        .context("getSlot")?;

    if ctx.json {
        return print_json(&result);
    }

    let empty = result["empty"].as_bool().unwrap_or(true);
    let value = result["value"].as_u64().unwrap_or(0);
    let creation_id = result["creation_id"]
        .as_u64()
        .context("getSlot response missing creation_id")?;
    let owner = result["owner"].as_str().unwrap_or("?");

    section(&format!("Slot #{index}"));
    if empty {
        kv("Status", &c!(DIM, "empty (unspent / available)"));
    } else {
        kv("Status", &c!(GRN, "live UTXO"));
        kv2(
            "Value",
            &format!("{} NOID", noid_str(value)),
            &format!("({value} μNOID)"),
        );
        kv("Creation ID", &creation_id.to_string());
        kv("Owner", ctx.h(owner));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// New chain commands
// ---------------------------------------------------------------------------

async fn cmd_block_hash(ctx: &Ctx<'_>, height: u64) -> anyhow::Result<()> {
    let result = rpc(ctx, "getBlockHash", &[height.into()])
        .await
        .context("getBlockHash")?;
    if ctx.json {
        return print_json(&result);
    }
    match result.as_str() {
        Some(hash) => {
            section(&format!("Block #{height} hash"));
            kv("Hash", hash);
        }
        None => warn_msg(&format!("Block {height} not found")),
    }
    Ok(())
}

async fn cmd_block_header(ctx: &Ctx<'_>, height: u64) -> anyhow::Result<()> {
    let result = rpc(ctx, "getBlockHeader", &[height.into()])
        .await
        .context("getBlockHeader")?;
    if ctx.json {
        return print_json(&result);
    }
    if result.is_null() {
        warn_msg(&format!("Block {height} not found"));
        return Ok(());
    }
    section(&format!("Block header #{height}"));
    kv("hash", result["hash"].as_str().unwrap_or("?"));
    kv("prev_hash", result["prev_hash"].as_str().unwrap_or("?"));
    kv(
        "height",
        &result["height"].as_u64().unwrap_or(0).to_string(),
    );
    kv(
        "timestamp",
        &result["timestamp"].as_u64().unwrap_or(0).to_string(),
    );
    kv("miner", result["miner"].as_str().unwrap_or("?"));
    kv("state_root", result["state_root"].as_str().unwrap_or("?"));
    kv("tx_root", result["tx_root"].as_str().unwrap_or("?"));
    kv(
        "difficulty_target",
        result["difficulty_target"].as_str().unwrap_or("?"),
    );
    kv(
        "active_slot_count",
        &result["active_slot_count"]
            .as_u64()
            .unwrap_or(0)
            .to_string(),
    );
    kv(
        "log_slots",
        &result["log_slots"].as_u64().unwrap_or(0).to_string(),
    );
    kv(
        "alloc_counter",
        &result["alloc_counter"].as_u64().unwrap_or(0).to_string(),
    );
    Ok(())
}

async fn cmd_utxos_of(ctx: &Ctx<'_>, address: &str) -> anyhow::Result<()> {
    let result = rpc(ctx, "getSlotsByOwner", &[address.into()])
        .await
        .context("getSlotsByOwner")?;
    if ctx.json {
        return print_json(&result);
    }
    let slots = result.as_array().cloned().unwrap_or_default();
    section(&format!("UTXOs of {address}"));
    if slots.is_empty() {
        println!("  {}", c!(DIM, "(no UTXOs found for this address)"));
        return Ok(());
    }
    let total: u64 = slots.iter().map(|s| s["value"].as_u64().unwrap_or(0)).sum();
    separator(74);
    println!("  {:<12}  {:>20}  {:>14}", "slot", "creation id", "NOID");
    separator(74);
    for s in &slots {
        let slot = s["slot_index"].as_u64().unwrap_or(0);
        let creation_id = s["creation_id"]
            .as_u64()
            .context("getSlotsByOwner response missing creation_id")?;
        let value = s["value"].as_u64().unwrap_or(0);
        println!(
            "  {:<12}  {:>20}  {:>14}",
            slot,
            creation_id,
            noid_str(value)
        );
    }
    separator(74);
    println!(
        "  {:<12}  {:>20}  {:>14}  ({} UTXOs)",
        "TOTAL",
        "",
        noid_str(total),
        slots.len()
    );
    Ok(())
}

async fn cmd_tx(ctx: &Ctx<'_>, txhash: &str) -> anyhow::Result<()> {
    let result = rpc(ctx, "getTx", &[txhash.into()]).await.context("getTx")?;
    if ctx.json {
        return print_json(&result);
    }
    if result.is_null() {
        warn_msg(&format!("Transaction {txhash} not found (not confirmed)"));
        println!("  Use 'parano1d-cli mempool-tx {txhash}' to check if it is pending.");
        return Ok(());
    }
    section("Transaction");
    kv("tx_hash", result["tx_hash"].as_str().unwrap_or("?"));
    kv(
        "height",
        &result["height"].as_u64().unwrap_or(0).to_string(),
    );
    kv("block_hash", result["block_hash"].as_str().unwrap_or("?"));
    kv(
        "position",
        &result["tx_position"].as_u64().unwrap_or(0).to_string(),
    );
    Ok(())
}

async fn cmd_state(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    let result = rpc(ctx, "getStateInfo", &[])
        .await
        .context("getStateInfo")?;
    if ctx.json {
        return print_json(&result);
    }

    let log_slots = result["log_slots"].as_u64().unwrap_or(0);
    let capacity = result["capacity"].as_u64().unwrap_or(0);
    let active = result["active_slots"].as_u64().unwrap_or(0);
    let fill_pct = result["fill_pct"].as_f64().unwrap_or(0.0);
    let headroom = result["slots_until_expand"].as_i64().unwrap_or(0);
    let trigger_pct = result["expand_trigger_pct"].as_u64().unwrap_or(75);
    let log_max = result["log_slots_max"].as_u64().unwrap_or(32);
    let size_human = result["state_size_human"].as_str().unwrap_or("?");

    section("UTXO state");
    kv2(
        "Slot space",
        &format!("2^{log_slots} = {capacity} slots"),
        &format!("(max 2^{log_max})"),
    );
    kv2(
        "Active UTXOs",
        &format!("{active}"),
        &format!("({fill_pct:.2}% full)"),
    );

    // Visual fill bar  [████████░░░░░░░░░░░░]  12.50%
    let bar_width = 30usize;
    let filled = ((fill_pct / 100.0) * bar_width as f64).round() as usize;
    let trigger_pos = ((trigger_pct as f64 / 100.0) * bar_width as f64).round() as usize;
    let bar: String = (0..bar_width)
        .map(|i| {
            if i < filled {
                '█'
            } else if i == trigger_pos.min(bar_width - 1) {
                '|'
            }
            // expansion marker
            else {
                '░'
            }
        })
        .collect();
    if is_tty() {
        println!(
            "  {CYN}{:<14}{RST} [{bar}] {fill_pct:.2}%  {DIM}(| = sample at {trigger_pct}%){RST}",
            "Fill",
            CYN = "\x1b[36m",
            RST = "\x1b[0m",
            DIM = "\x1b[2m"
        );
    } else {
        println!(
            "  {:<14} [{bar}] {fill_pct:.2}%  (| = sample at {trigger_pct}%)",
            "Fill"
        );
    }

    if headroom >= 0 {
        kv2(
            "Until expand",
            &format!("{headroom} slots"),
            &format!(
                "({:.2}% headroom)",
                headroom as f64 / capacity as f64 * 100.0
            ),
        );
    } else {
        kv(
            "Until expand",
            &c!(
                YLW,
                "ABOVE SAMPLE THRESHOLD (awaiting finalized-window majority)"
            ),
        );
    }
    kv("State storage", size_human);
    Ok(())
}

async fn cmd_mining(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    let result = rpc(ctx, "getMiningInfo", &[])
        .await
        .context("getMiningInfo")?;
    if ctx.json {
        return print_json(&result);
    }
    let height = result["height"].as_u64().unwrap_or(0);
    let diff_bits = result["difficulty_bits"].as_u64().unwrap_or(0);
    let diff_target = result["difficulty_target"].as_str().unwrap_or("?");
    let reward_micro = result["block_reward_micronoid"].as_u64().unwrap_or(0);
    let active = result["active_slot_count"].as_u64().unwrap_or(0);
    section("Mining info");
    kv("Height", &height.to_string());
    kv2(
        "Difficulty",
        &format!("{diff_bits} leading zeros"),
        &format!("target: {diff_target}"),
    );
    kv2(
        "Block reward",
        &format!("{} NOID/block", noid_str(reward_micro)),
        &format!("({reward_micro} \u{03bc}NOID)"),
    );
    kv("Active UTXOs", &active.to_string());
    Ok(())
}

async fn cmd_peers(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    let result = rpc(ctx, "getPeerCount", &[])
        .await
        .context("getPeerCount")?;
    if ctx.json {
        return print_json(&result);
    }
    let n = result.as_u64().unwrap_or(0);
    section("Connected peers");
    kv("Count", &n.to_string());
    Ok(())
}

async fn cmd_estimate_fee(ctx: &Ctx<'_>, n_inputs: u32, n_outputs: u32) -> anyhow::Result<()> {
    let result = rpc(
        ctx,
        "estimateFeeDetailed",
        &[n_inputs.into(), n_outputs.into()],
    )
    .await
    .context("estimateFeeDetailed")?;
    if ctx.json {
        return print_json(&result);
    }
    let fee_micro = result["fee_micronoid"].as_u64().unwrap_or(0);
    let b = &result["breakdown"];
    section(&format!(
        "Fee estimate ({n_inputs} input(s), {n_outputs} output(s))"
    ));
    kv2(
        "Min relay fee",
        &format!("{} NOID", noid_str(fee_micro)),
        &format!("({fee_micro} μNOID)"),
    );
    kv(
        "Base",
        &format!("{} μNOID", b["base"].as_u64().unwrap_or(0)),
    );
    kv(
        "Inputs",
        &format!("{} μNOID", b["input"].as_u64().unwrap_or(0)),
    );
    kv(
        "Outputs",
        &format!("{} μNOID", b["output"].as_u64().unwrap_or(0)),
    );
    kv(
        "State growth burned",
        &format!("{} μNOID", b["state_growth"].as_u64().unwrap_or(0)),
    );
    kv(
        "Miner claimable",
        &format!("{} μNOID", b["miner_claimable"].as_u64().unwrap_or(0)),
    );
    println!();
    println!(
        "  {} output-centric: base + small input anti-DoS + output fee + burned net-new-state fee",
        c!(DIM, "Formula:")
    );
    Ok(())
}

async fn cmd_validate(ctx: &Ctx<'_>, address: &str) -> anyhow::Result<()> {
    let result = rpc(ctx, "validateAddress", &[address.into()])
        .await
        .context("validateAddress")?;
    if ctx.json {
        return print_json(&result);
    }
    let valid = result["valid"].as_bool().unwrap_or(false);
    section("Address validation");
    if valid {
        ok_msg("Valid address");
        kv("bech32m", result["bech32"].as_str().unwrap_or("?"));
        kv("hex", result["hex"].as_str().unwrap_or("?"));
    } else {
        let err = result["error"].as_str().unwrap_or("invalid");
        print_error(&format!("Invalid address: {err}"));
        bail!("invalid address");
    }
    Ok(())
}

async fn cmd_mempool_tx(ctx: &Ctx<'_>, txhash: &str) -> anyhow::Result<()> {
    let result = rpc(ctx, "getMempoolEntry", &[txhash.into()])
        .await
        .context("getMempoolEntry")?;
    if ctx.json {
        return print_json(&result);
    }
    if result.is_null() {
        warn_msg(&format!("Transaction {txhash} is not in the mempool."));
        println!("  It may have been confirmed. Use 'parano1d-cli tx {txhash}' to check.");
        return Ok(());
    }
    section("Mempool transaction");
    kv("tx_hash", result["tx_hash"].as_str().unwrap_or("?"));
    let fee = result["fee_micronoid"].as_u64().unwrap_or(0);
    kv2(
        "Fee",
        &format!("{} NOID", noid_str(fee)),
        &format!("({fee} \u{03bc}NOID)"),
    );
    kv(
        "Inputs",
        &result["n_inputs"].as_u64().unwrap_or(0).to_string(),
    );
    kv(
        "Outputs",
        &result["n_outputs"].as_u64().unwrap_or(0).to_string(),
    );
    kv(
        "Physical pages",
        &result["page_count"].as_u64().unwrap_or(0).to_string(),
    );
    kv(
        "Minimum proof class",
        result["minimum_proof_class"].as_str().unwrap_or("?"),
    );
    kv(
        "Admitted at height",
        &result["admitted_height"].as_u64().unwrap_or(0).to_string(),
    );
    let has_authorization = result["has_authorization"].as_bool().unwrap_or(false);
    kv(
        "Authorization",
        if has_authorization {
            "attached"
        } else {
            "not attached"
        },
    );
    if result["requires_b255_miner"].as_bool().unwrap_or(false) {
        println!();
        warn_msg("Waiting for inclusion: this atomic PagedSpend requires a B255-qualified miner.");
    }
    Ok(())
}

async fn cmd_epoch(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    let result = rpc(ctx, "getEpochAnchor", &[])
        .await
        .context("getEpochAnchor")?;

    if ctx.json {
        return print_json(&result);
    }

    let hash = result.as_str().unwrap_or("?");
    section("Epoch anchor");
    kv("Hash", ctx.h(hash));
    println!();
    println!(
        "  {} Exact transaction-epoch anchor accepted for the next block.",
        c!(DIM, "Note:")
    );

    Ok(())
}

async fn cmd_mempool(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    let result = rpc(ctx, "getMempoolInfo", &[])
        .await
        .context("getMempoolInfo")?;

    if ctx.json {
        return print_json(&result);
    }

    let size = result["size"].as_u64().unwrap_or(0);
    let fee_floor = result["fee_floor"].as_u64().unwrap_or(0);
    let txs = result["txs"].as_array().cloned().unwrap_or_default();

    section("Mempool");
    kv2("Pending", &size.to_string(), "transactions");
    kv2(
        "Fee floor",
        &format!("{} NOID", noid_str(fee_floor)),
        &format!("({fee_floor} μNOID minimum)"),
    );

    if txs.is_empty() {
        println!();
        println!("  {}", c!(DIM, "(mempool is empty)"));
        return Ok(());
    }

    println!();
    separator(100);
    if is_tty() {
        println!(
            "  {}{:<20}  {:>12}  {:>5}  {:>3}→{:<3}  {:>5}  {}{}",
            BOLD, "tx hash", "fee (μNOID)", "pages", "in", "out", "class", "proof", RST
        );
    } else {
        println!(
            "  {:<20}  {:>12}  {:>5}  {:>3}→{:<3}  {:>5}  {}",
            "tx hash", "fee (μNOID)", "pages", "in", "out", "class", "proof"
        );
    }
    separator(100);

    let show = txs.len().min(20);
    for tx in txs.iter().take(show) {
        let hash = tx["tx_hash"].as_str().unwrap_or("?");
        let fee = tx["fee_micronoid"].as_u64().unwrap_or(0);
        let pages = tx["page_count"].as_u64().unwrap_or(0);
        let nin = tx["n_inputs"].as_u64().unwrap_or(0);
        let nout = tx["n_outputs"].as_u64().unwrap_or(0);
        let class = tx["minimum_proof_class"].as_str().unwrap_or("?");
        let authorization = if tx["has_authorization"].as_bool().unwrap_or(false) {
            c!(GRN, "✓")
        } else {
            c!(DIM, "·")
        };
        println!(
            "  {:<20}  {:>12}  {:>5}  {:>3}→{:<3}  {:>5}  {}",
            ctx.h(hash),
            fee,
            pages,
            nin,
            nout,
            class,
            authorization
        );
    }

    if txs.len() > show {
        println!("  {} {} more…", c!(DIM, "...and"), txs.len() - show);
    }
    if txs
        .iter()
        .any(|tx| tx["requires_b255_miner"].as_bool().unwrap_or(false))
    {
        println!();
        warn_msg("B255 entries are atomic and wait for a B255-qualified miner.");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Wallet commands
// ---------------------------------------------------------------------------

async fn cmd_address(
    ctx: &Ctx<'_>,
    new: bool,
    list: bool,
    index: Option<u32>,
    use_index: Option<u32>,
) -> anyhow::Result<()> {
    if let Some(idx) = use_index {
        let result = rpc(ctx, "walletSetActiveAddress", &[serde_json::json!(idx)])
            .await
            .context("walletSetActiveAddress")?;
        if ctx.json {
            return print_json(&result);
        }
        let addr = result["address"].as_str().unwrap_or("?");
        let bal = rpc(ctx, "walletGetBalance", &[])
            .await
            .ok()
            .and_then(|balance| balance["balance_noid"].as_f64())
            .unwrap_or(0.0);
        section(&format!("Active address switched [index={idx}]"));
        if is_tty() {
            println!("  {}{}{}", BOLD, addr, RST);
        } else {
            println!("  {}", addr);
        }
        println!("  Balance: {bal:.6} NOID");
        println!();
        println!(
            "  {} Sends now spend from this address only; change returns here.",
            c!(DIM, "↑")
        );
        return Ok(());
    }
    if list {
        let result = rpc(ctx, "walletListAddresses", &[])
            .await
            .context("walletListAddresses")?;
        if ctx.json {
            return print_json(&result);
        }
        let addrs = result.as_array().cloned().unwrap_or_default();
        section("Wallet addresses");
        println!("  {:>7}  {}", "index", "address");
        separator(72);
        for a in &addrs {
            let idx = a["key_index"].as_u64().unwrap_or(0);
            let addr = a["address"].as_str().unwrap_or("");
            let active = a["is_active"].as_bool().unwrap_or(false);
            let marker = if active { "▶" } else { " " };
            println!("  {} {:>5}  {}", marker, idx, ctx.h(addr));
        }
        separator(72);
        println!("  {} locally generated address(es)", addrs.len());
        println!();
        println!(
            "  {}",
            c!(
                DIM,
                "▶ = active address (sends spend from it). Switch: 'address --use <index>'."
            )
        );
        println!(
            "  {}",
            c!(
                DIM,
                "To move funds between your addresses, activate the source and send normally."
            )
        );
    } else if new {
        let result = rpc(ctx, "walletNextAddress", &[])
            .await
            .context("walletNextAddress")?;
        if ctx.json {
            return print_json(&result);
        }
        let addr = result["address"].as_str().unwrap_or("");
        let idx = result["key_index"].as_u64().unwrap_or(0);
        section(&format!("New address [index={idx}]"));
        if is_tty() {
            println!("  {}{}{}", BOLD, addr, RST);
        } else {
            println!("  {}", addr);
        }
        println!();
        println!(
            "  {} The active address is unchanged. Activate this address with 'address --use {idx}'.",
            c!(DIM, "↑")
        );
    } else if let Some(idx) = index {
        let result = rpc(ctx, "walletGetAddress", &[serde_json::json!(idx)])
            .await
            .context("walletGetAddress")?;
        if ctx.json {
            return print_json(&result);
        }
        let addr = result.as_str().unwrap_or("?");
        section(&format!("Wallet address [index={idx}]"));
        if is_tty() {
            println!("  {}{}{}", BOLD, addr, RST);
        } else {
            println!("  {}", addr);
        }
    } else {
        // default: the ACTIVE address.
        let result = rpc(ctx, "walletActiveAddress", &[])
            .await
            .context("walletActiveAddress")?;
        if ctx.json {
            return print_json(&result);
        }
        let addr = result["address"].as_str().unwrap_or("?");
        let idx = result["key_index"].as_u64().unwrap_or(0);
        section(&format!("Active address [index={idx}]"));
        if is_tty() {
            println!("  {}{}{}", BOLD, addr, RST);
        } else {
            println!("  {}", addr);
        }
        println!();
        println!(
            "  {} Sends spend from this address; share it to receive NOID here.",
            c!(DIM, "↑")
        );
        println!(
            "  {}Tip: 'address --new' for a fresh address, '--list' for all, '--use <i>' to switch.{}",
            DIM, RST
        );
    }
    Ok(())
}

async fn cmd_balance(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    let result = rpc(ctx, "walletGetBalance", &[])
        .await
        .context("walletGetBalance")?;

    if ctx.json {
        return print_json(&result);
    }

    let micro = result["balance_micronoid"].as_u64().unwrap_or(0);
    let utxos = result["utxo_count"].as_u64().unwrap_or(0);
    let pending_out = result["pending_outbound_micronoid"].as_u64().unwrap_or(0);
    let spendable = result["spendable_micronoid"]
        .as_u64()
        .unwrap_or_else(|| micro.saturating_sub(pending_out));

    section("Wallet balance (active address)");
    if is_tty() {
        println!(
            "  {}Balance:{} {}{} NOID{} {}({} \u{03bc}NOID){}  {}({} UTXOs){}",
            CYN,
            RST,
            BOLD,
            noid_str(micro),
            RST,
            DIM,
            micro,
            RST,
            DIM,
            utxos,
            RST
        );
    } else {
        println!(
            "  Balance:           {} NOID  ({micro} \u{03bc}NOID)  ({utxos} UTXOs)",
            noid_str(micro)
        );
    }

    if pending_out > 0 {
        if is_tty() {
            println!(
                "  {}Pending:{}  -{} NOID outbound  {}({} \u{03bc}NOID locked){}",
                YLW,
                RST,
                noid_str(pending_out),
                DIM,
                pending_out,
                RST
            );
            println!(
                "  {}Spendable:{} {}{} NOID{}",
                CYN,
                RST,
                BOLD,
                noid_str(spendable),
                RST
            );
        } else {
            println!(
                "  Pending:           -{} NOID outbound ({pending_out} \u{03bc}NOID locked)",
                noid_str(pending_out)
            );
            println!("  Spendable:         {} NOID", noid_str(spendable));
        }
    }

    if micro == 0 && utxos == 0 {
        println!();
        warn_msg("No UTXOs found on the active address.");
        println!(
            "       Run {} to reload the active address; use {} to switch accounts.",
            c!(BOLD, "'parano1d-cli scan'"),
            c!(BOLD, "'address --list'")
        );
    }

    Ok(())
}

async fn cmd_utxos(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    let result = rpc(ctx, "walletListUtxos", &[])
        .await
        .context("walletListUtxos")?;

    if ctx.json {
        return print_json(&result);
    }

    let utxos = result.as_array().cloned().unwrap_or_default();

    section("Wallet UTXOs");

    if utxos.is_empty() {
        println!(
            "  {}",
            c!(DIM, "(no UTXOs — run 'parano1d-cli scan' to discover)")
        );
        return Ok(());
    }

    // Compute total
    let total: u64 = utxos
        .iter()
        .map(|u| u["value_micronoid"].as_u64().unwrap_or(0))
        .sum();

    separator(96);
    if is_tty() {
        println!(
            "  {}{:<8}  {:>20}  {:>14}  {:>7}  {:>9}  {}{}",
            BOLD, "slot", "creation id", "NOID", "key", "at block", "address", RST
        );
    } else {
        println!(
            "  {:<8}  {:>20}  {:>14}  {:>7}  {:>9}  {}",
            "slot", "creation id", "NOID", "key", "at block", "address"
        );
    }
    separator(96);

    for u in &utxos {
        let slot = u["slot_index"].as_u64().unwrap_or(0);
        let creation_id = u["creation_id"]
            .as_u64()
            .context("walletListUtxos response missing creation_id")?;
        let micro = u["value_micronoid"].as_u64().unwrap_or(0);
        let key = u["key_index"].as_u64().unwrap_or(0);
        let height = u["confirmed_height"].as_u64().unwrap_or(0);
        let addr = u["address"].as_str().unwrap_or("?");
        println!(
            "  {:<8}  {:>20}  {:>14}  {:>7}  {:>9}  {}",
            slot,
            creation_id,
            noid_str(micro),
            key,
            height,
            ctx.h(addr)
        );
    }

    separator(96);
    if is_tty() {
        println!(
            "  {}{:<8}  {:>20}  {:>14}{}",
            BOLD,
            "TOTAL",
            "",
            noid_str(total),
            RST
        );
    } else {
        println!("  {:<8}  {:>20}  {:>14}", "TOTAL", "", noid_str(total));
    }

    Ok(())
}

async fn cmd_send(
    ctx: &Ctx<'_>,
    to: &str,
    amount: &str,
    fee: Option<&str>,
    dry_run: bool,
) -> anyhow::Result<()> {
    // --- Parse and validate inputs ---
    let amount_micro =
        parse_noid_amount(amount).with_context(|| format!("invalid amount {amount:?}"))?;

    if amount_micro == 0 {
        bail!("Amount cannot be zero.");
    }

    // Warn if the amount looks suspiciously large (> 1 000 000 NOID = 1e12 μNOID).
    // This catches the common mistake of passing μNOID to a NOID-denomination CLI.
    const MAX_WARN_MICRO: u64 = 1_000_000 * 1_000_000; // 1M NOID in μNOID
    if amount_micro > MAX_WARN_MICRO {
        eprintln!(
            "⚠  Large amount: {} NOID ({} μNOID). \
             Note: this CLI takes NOID, not μNOID. Press Ctrl-C to cancel.",
            noid_str(amount_micro),
            amount_micro
        );
    }

    let fee_micro = match fee {
        Some(f) => parse_noid_amount(f).with_context(|| format!("invalid fee {f:?}"))?,
        None => 0, // auto
    };

    // Validate address encoding before sending to the daemon.
    // Actual parsing/validation happens in the daemon; we just do a basic
    // sanity check to catch obvious typos before sending to RPC.
    let to_clean = to.trim();
    let looks_like_bech32 = to_clean.to_ascii_lowercase().starts_with("o1");
    if !looks_like_bech32 {
        bail!(
            "Invalid address format.\n\
             \tExpected: bech32m address (o1…)\n\
             \tGot:      {:?}\n\
             \tExample:  o1q9gnyj0z…",
            &to_clean[..to_clean.len().min(30)]
        );
    }

    if dry_run {
        let result = match rpc(
            ctx,
            "walletPlanSend",
            &[to_clean.into(), amount_micro.into(), fee_micro.into()],
        )
        .await
        {
            Ok(result) => result,
            Err(error) => {
                if let Some(message) = input_limit_exceeded_message(&error) {
                    bail!(message);
                }
                return Err(error).context("walletPlanSend");
            }
        };
        if ctx.json {
            return print_json(&result);
        }
        section("Wallet send plan");
        kv("To", to_clean);
        kv2(
            "Amount",
            &format!("{} NOID", noid_str(amount_micro)),
            &format!("({amount_micro} μNOID)"),
        );
        let planned_fee = result["fee_micronoid"].as_u64().unwrap_or(0);
        kv2(
            "Fee",
            &format!("{} NOID", noid_str(planned_fee)),
            &format!(
                "({planned_fee} μNOID){}",
                if fee.is_none() { " auto" } else { "" }
            ),
        );
        kv(
            "Inputs",
            &result["input_count"].as_u64().unwrap_or(0).to_string(),
        );
        kv(
            "Outputs",
            &result["output_count"].as_u64().unwrap_or(0).to_string(),
        );
        let change = result["change_micronoid"].as_u64().unwrap_or(0);
        kv2(
            "Change",
            &format!("{} NOID", noid_str(change)),
            &format!("({change} μNOID)"),
        );
        println!();
        println!(
            "  {} Dry run only; no proof was generated and nothing was submitted.",
            c!(DIM, "Note:")
        );
        return Ok(());
    }

    // --- Confirm interactively for large amounts ---
    if amount_micro >= 1_000_000_000 /* 1000 NOID */ && is_tty() {
        print!(
            "  {} Send {}{} NOID{} to {}{}{}? [y/N] ",
            c!(YLW, "⚠"),
            BOLD,
            noid_str(amount_micro),
            RST,
            CYN,
            to_clean,
            RST,
        );
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("  {}", c!(DIM, "Cancelled."));
            return Ok(());
        }
    }

    // --- Send ---
    let result = rpc(
        ctx,
        "walletSend",
        &[to_clean.into(), amount_micro.into(), fee_micro.into()],
    )
    .await;

    match result {
        Ok(r) if ctx.json => return print_json(&r),
        Ok(r) => {
            let actual_fee = r["fee_micronoid"].as_u64().unwrap_or(0);
            let auto_tag = if fee.is_none() { " (auto)" } else { "" };

            section("Transaction submitted");
            ok_msg(&format!("TX {}", ctx.h(r["txid"].as_str().unwrap_or("?"))));
            println!();
            kv("To", to_clean);
            kv2(
                "Amount",
                &format!("{} NOID", noid_str(amount_micro)),
                &format!("({amount_micro} μNOID)"),
            );
            kv2(
                "Fee",
                &format!("{} NOID", noid_str(actual_fee)),
                &format!("({actual_fee} μNOID){auto_tag}"),
            );
            println!();
            println!(
                "  {} Submitted transaction is pending confirmation.",
                c!(DIM, "⏳")
            );
            println!(
                "  {} Use {} to check your balance after confirmation.",
                c!(DIM, "Tip:"),
                c!(BOLD, "'parano1d-cli balance'")
            );
        }
        Err(e) => {
            // Re-format common wallet errors into human language
            let msg = e.to_string();
            if let Some(message) = input_limit_exceeded_message(&e) {
                bail!(message);
            }
            let human = if msg.contains("Insufficient") || msg.contains("insufficient") {
                // Try to extract amounts
                format!(
                    "Insufficient funds.\n\
                     \t  Requested: {} NOID  ({amount_micro} μNOID)\n\
                     \t  Run 'parano1d-cli balance' to check your current balance.",
                    noid_str(amount_micro)
                )
            } else if msg.contains("no UTXO") || msg.contains("no utxo") {
                "No UTXOs available. Run 'parano1d-cli scan' to discover your coins.".into()
            } else if msg.contains("no empty slot hints") {
                "No empty output slots are currently available. This is usually transient; wait for the next block and retry."
                    .into()
            } else if msg.contains("output slot") || msg.contains("SlotConflict") {
                "Slot conflict: the output slot is occupied. This is transient — retry in a moment."
                    .into()
            } else if msg.contains("BelowMinFee") {
                format!(
                    "Fee is below the current network minimum. Retry without --fee for automatic fee selection.\n\tDetails: {msg}"
                )
            } else if msg.contains("proof") || msg.contains("prove") || msg.contains("task:") {
                format!(
                    "Proof generation failed. Retry once; if it repeats, save the logs and report it.\n\tDetails: {msg}"
                )
            } else {
                msg
            };
            bail!("{human}");
        }
    }

    Ok(())
}

async fn cmd_history(
    ctx: &Ctx<'_>,
    address_filter: Option<&str>,
    last: Option<usize>,
) -> anyhow::Result<()> {
    let result = rpc(ctx, "walletHistory", &[])
        .await
        .context("walletHistory")?;

    if ctx.json {
        return print_json(&result);
    }

    let entries = result.as_array().cloned().unwrap_or_default();

    // Apply address filter
    let mut filtered: Vec<&Value> = entries
        .iter()
        .filter(|e| {
            if let Some(filter) = address_filter {
                e["own_address"]
                    .as_str()
                    .map(|a| a == filter)
                    .unwrap_or(false)
                    || e["peer_address"]
                        .as_str()
                        .map(|a| a == filter)
                        .unwrap_or(false)
            } else {
                true
            }
        })
        .collect();

    // Apply --last N limit
    if let Some(n) = last {
        let len = filtered.len();
        if len > n {
            filtered = filtered[len - n..].to_vec();
        }
    }

    section("Transaction history");

    if filtered.is_empty() {
        println!("  {}", c!(DIM, "(no transactions yet)"));
        return Ok(());
    }

    separator(88);
    if is_tty() {
        println!(
            "  {}  {:<8}  {:<8}  {:>14}  {:<16}  {}{}",
            BOLD, "block", "dir", "NOID", "own[idx]", "counterparty", RST
        );
    } else {
        println!(
            "  {:<8}  {:<8}  {:>14}  {:<16}  {}",
            "block", "dir", "NOID", "own[idx]", "counterparty"
        );
    }
    separator(88);

    // Compute totals
    let mut sent: u64 = 0;
    let mut received: u64 = 0;

    for e in &filtered {
        let height = e["height"].as_u64().unwrap_or(0);
        let dir = e["direction"].as_str().unwrap_or("?");
        let micro = e["amount_micronoid"].as_u64().unwrap_or(0);

        let (sign, arrow, colour) = if dir == "received" || dir == "recv" {
            ("+", "\u{2190} recv", GRN)
        } else {
            ("-", "\u{2192} sent", RED)
        };

        if dir == "sent" {
            sent += micro;
        } else {
            received += micro;
        }

        let own = e["own_address"].as_str().unwrap_or("");
        let own_idx = e["own_key_index"].as_u64();
        let own_display = if let Some(idx) = own_idx {
            let truncated = &own[..own.len().min(10)];
            format!("[{}]{}", idx, truncated)
        } else {
            String::new()
        };
        let peer = e["peer_address"].as_str().unwrap_or("\u{2014}");
        let amount_str = format!("{:>14}{}", noid_str(micro), sign);

        if is_tty() {
            println!(
                "  {:>8}  {:<8}  {}{}{}  {:<16}  {}",
                height,
                arrow,
                colour,
                amount_str,
                RST,
                own_display,
                ctx.h(peer)
            );
        } else {
            println!(
                "  {:>8}  {:<8}  {}  {:<16}  {}",
                height, arrow, amount_str, own_display, peer
            );
        }
    }

    separator(88);
    if is_tty() {
        println!(
            "  {}  {:<8}  {:<8}  {}{}  {}{}{}",
            BOLD,
            "",
            "",
            GRN,
            format!("+ {} NOID received", noid_str(received)),
            RED,
            format!("  - {} NOID sent", noid_str(sent)),
            RST
        );
    } else {
        println!(
            "  total: +{} received  -{} sent",
            noid_str(received),
            noid_str(sent)
        );
    }

    Ok(())
}

async fn cmd_scan(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    if is_tty() {
        eprint!("  Reloading active address...");
        io::stderr().flush()?;
    }

    let result = rpc(ctx, "walletScan", &[]).await.context("walletScan")?;

    if is_tty() {
        eprintln!(" done.");
    }

    if ctx.json {
        return print_json(&result);
    }

    let found = result["found_utxos"].as_u64().unwrap_or(0);
    let balance_micronoid = result["balance_micronoid"].as_u64().unwrap_or(0);
    let active_index = result["active_index"].as_u64().unwrap_or(0);
    let snapshot_height = result["snapshot_height"].as_u64().unwrap_or(0);

    section("Active address reloaded");
    println!(
        "  Index {}  \u{2022}  Snapshot height {}  \u{2022}  Found {} UTXO(s)  \u{2022}  Balance: {} NOID",
        active_index,
        snapshot_height,
        found,
        noid_str(balance_micronoid)
    );

    if found == 0 {
        println!();
        warn_msg(
            "No UTXOs found. If you expect a balance, check that you're using the right data directory.",
        );
    }

    Ok(())
}

async fn cmd_receipt(ctx: &Ctx<'_>, txhash: &str) -> anyhow::Result<()> {
    let result = rpc(ctx, "walletExportReceipt", &[txhash.into()])
        .await
        .context("walletExportReceipt")?;

    let hex = result
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("daemon returned non-string receipt"))?;

    // Raw hex to stdout so it can be redirected
    println!("{hex}");

    if is_tty() {
        // If outputting to terminal, also show a tip
        eprintln!();
        eprintln!(
            "  {} Redirect to a file: parano1d-cli receipt {} > receipt.hex",
            c!(DIM, "Tip:"),
            txhash
        );
        eprintln!(
            "  {} Verify:              parano1d-cli verify $(cat receipt.hex)",
            c!(DIM, "Tip:")
        );
    }

    Ok(())
}

async fn cmd_verify(ctx: &Ctx<'_>, receipt: &str) -> anyhow::Result<()> {
    let result = rpc(ctx, "verifyReceipt", &[receipt.into()])
        .await
        .context("verifyReceipt")?;

    if ctx.json {
        return print_json(&result);
    }

    let merkle_valid = result["merkle_valid"].as_bool().unwrap_or(false);
    let canonical = result["canonical"].as_bool().unwrap_or(false);
    let confirmed = result["confirmed"].as_bool().unwrap_or(false);
    let error = result["error"].as_str();
    let authenticated_summary = result["authenticated_summary"].as_object();

    section("Receipt verification");

    if confirmed {
        ok_msg("Receipt is VALID and canonical.");
        kv(
            "Merkle proof",
            if merkle_valid {
                "✓ valid"
            } else {
                "✗ invalid"
            },
        );
        kv(
            "On canonical chain",
            if canonical { "✓ yes" } else { "✗ no" },
        );
    } else {
        let reason = error.unwrap_or("receipt is not confirmed on the canonical chain");
        print_error(&format!("Receipt INVALID: {reason}"));
        kv(
            "Merkle proof",
            if merkle_valid {
                "✓ valid"
            } else {
                "✗ invalid"
            },
        );
        kv(
            "On canonical chain",
            if canonical {
                "✓ yes"
            } else {
                "✗ no (block may have been reorged)"
            },
        );
        bail!("Receipt verification failed");
    }

    if let Some(summary) = authenticated_summary {
        let txid = summary
            .get("txid")
            .and_then(|value| value.as_str())
            .unwrap_or("?");
        let height = summary
            .get("claimed_height")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let confirmed_unix = summary
            .get("confirmed_unix")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let fee = summary
            .get("fee_micronoid")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let inputs = summary
            .get("inputs")
            .and_then(|value| value.as_array())
            .map_or(0, Vec::len);
        let outputs = summary
            .get("outputs")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();

        println!();
        section("Authenticated payment");
        kv("Transaction", txid);
        kv2(
            "Block",
            &format!("#{height}"),
            &format!("unix {confirmed_unix}"),
        );
        kv("Fee", &format!("{} ({} μNOID)", noid_str(fee), fee));
        kv("Inputs", &inputs.to_string());
        kv("Outputs", &outputs.len().to_string());
        for (index, output) in outputs.iter().enumerate() {
            let owner = output["owner"].as_str().unwrap_or("?");
            let amount = output["amount_micronoid"].as_u64().unwrap_or(0);
            let slot = output["slot_index"].as_u64().unwrap_or(0);
            kv2(
                &format!("  Output {}", index + 1),
                &format!("{} ({} μNOID)", noid_str(amount), amount),
                &format!("to {owner}, slot {slot}"),
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Node control
// ---------------------------------------------------------------------------

async fn cmd_stop(ctx: &Ctx<'_>) -> anyhow::Result<()> {
    match rpc(ctx, "stop", &[]).await {
        Ok(r) => {
            let msg = r.as_str().unwrap_or("shutting down");
            ok_msg(&format!("Daemon is {msg}."));
        }
        Err(_) => {
            // Connection dropped = daemon already shutting down (expected)
            ok_msg("Daemon is shutting down.");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Mining commands (external miner API)
// ---------------------------------------------------------------------------

async fn cmd_block_template(ctx: &Ctx<'_>, miner_addr: &str) -> anyhow::Result<()> {
    let result = rpc(ctx, "getBlockTemplate", &[miner_addr.into()])
        .await
        .context("getBlockTemplate")?;

    if ctx.json {
        return print_json(&result);
    }

    let height = result["height"].as_u64().unwrap_or(0);
    let n_txs = result["n_txs"].as_u64().unwrap_or(0);
    let claimable_fees = result["claimable_fees_micronoid"].as_u64().unwrap_or(0);
    let coinbase_value = result["coinbase_value_micronoid"].as_u64().unwrap_or(0);
    let template_id = result["template_id"].as_str().unwrap_or("");
    let expires_in_seconds = result["expires_in_seconds"].as_u64().unwrap_or(0);
    let nonce_field_index = result["nonce_field_index"].as_u64().unwrap_or(0);
    let difficulty_target = result["difficulty_target_hex"].as_str().unwrap_or("");
    let pow_fields = result["pow_fields_hex"].as_str().unwrap_or("");

    section("Block template");
    kv("Template ID", template_id);
    kv("Height", &height.to_string());
    kv("Txs in block", &n_txs.to_string());
    kv("Expires in", &format!("{expires_in_seconds} s"));
    kv("Nonce field", &nonce_field_index.to_string());
    kv("Difficulty", difficulty_target);
    kv("Claimable fees", &format!("{claimable_fees} μNOID"));
    kv("Coinbase value", &format!("{coinbase_value} μNOID"));
    if !pow_fields.is_empty() {
        kv2(
            "PoW fields",
            &format!("{}…", &pow_fields[..pow_fields.len().min(32)]),
            "(16 field elements, Poseidon2b PoW input)",
        );
    }
    println!();
    println!(
        "  {} Search the nonce, then run `parano1d-cli submit-block {template_id} <32-char-lowercase-nonce-hex>`.",
        c!(DIM, "PoW:")
    );
    if !pow_fields.is_empty() {
        println!("  {} {}", c!(DIM, "Field schedule hex:"), pow_fields);
    }

    Ok(())
}

async fn cmd_submit_block(ctx: &Ctx<'_>, template_id: &str, nonce_hex: &str) -> anyhow::Result<()> {
    let result = rpc(ctx, "submitBlock", &[template_id.into(), nonce_hex.into()])
        .await
        .context("submitBlock")?;

    if ctx.json {
        return print_json(&result);
    }

    let hash = result.as_str().unwrap_or("?");
    ok_msg(&format!("Block accepted: {}", ctx.h(hash)));

    Ok(())
}

// ---------------------------------------------------------------------------
// JSON-RPC transport
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct RpcCallError {
    code: i64,
    message: String,
    data: Option<Value>,
}

impl std::fmt::Display for RpcCallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RPC error ({}): {}", self.code, self.message)
    }
}

impl std::error::Error for RpcCallError {}

fn input_limit_exceeded_message(error: &anyhow::Error) -> Option<String> {
    let rpc = error.downcast_ref::<RpcCallError>()?;
    if rpc.code != i64::from(noid_rpc::types::WALLET_INPUT_LIMIT_EXCEEDED_CODE) {
        return None;
    }
    let max_inputs = rpc
        .data
        .as_ref()
        .and_then(|data| data.get("max_inputs"))
        .and_then(Value::as_u64)
        .unwrap_or(8);
    Some(format!(
        "Payment cannot be created: it requires more than {max_inputs} inputs."
    ))
}

async fn rpc(ctx: &Ctx<'_>, method: &str, params: &[Value]) -> anyhow::Result<Value> {
    let method_full = format!("paranoid_{method}");
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id":      1,
        "method":  method_full,
        "params":  params,
    });

    let resp: Value = ctx
        .client
        .post(ctx.rpc)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                anyhow::anyhow!(
                    "Node is not responding.\n\
                     \tIs the parano1d daemon running?  Try: parano1d --mode miner\n\
                     \tRPC endpoint: {}\n\
                     \tOverride with --rpc <URL> or NOID_RPC env var",
                    ctx.rpc
                )
            } else {
                anyhow::anyhow!("HTTP error: {e}")
            }
        })?
        .json()
        .await
        .with_context(|| format!("decode JSON-RPC response for {method}"))?;

    if let Some(err) = resp.get("error") {
        let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        let message = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        return Err(anyhow::Error::new(RpcCallError {
            code,
            message: message.to_string(),
            data: err.get("data").cloned(),
        }));
    }

    Ok(resp["result"].clone())
}

fn print_json(v: &Value) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

#[cfg(test)]
mod amount_tests {
    use super::{input_limit_exceeded_message, noid_str, parse_noid_amount, RpcCallError};

    #[test]
    fn decimal_amounts_are_exact_and_checked() {
        assert_eq!(parse_noid_amount("10.5").unwrap(), 10_500_000);
        assert_eq!(parse_noid_amount(".000001").unwrap(), 1);
        assert_eq!(parse_noid_amount("1.").unwrap(), 1_000_000);
        assert_eq!(noid_str(u64::MAX), "18446744073709.551615");
        assert!(parse_noid_amount("0.0000001").is_err());
        assert!(parse_noid_amount("NaN").is_err());
        assert!(parse_noid_amount("inf").is_err());
        assert!(parse_noid_amount("18446744073710").is_err());
    }

    #[test]
    fn input_limit_error_is_concise_and_uses_the_structured_limit() {
        let error = anyhow::Error::new(RpcCallError {
            code: i64::from(noid_rpc::types::WALLET_INPUT_LIMIT_EXCEEDED_CODE),
            message: noid_rpc::types::WALLET_INPUT_LIMIT_EXCEEDED_MESSAGE.to_string(),
            data: Some(serde_json::json!({ "max_inputs": 8 })),
        });
        assert_eq!(
            input_limit_exceeded_message(&error).as_deref(),
            Some("Payment cannot be created: it requires more than 8 inputs.")
        );
    }
}
