// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Full recursive candidate measurements. No network daemon or release pins
//! are modified. Build with `--features noid_chain/isolated-v1-1-testnet` to
//! read the authenticated H1..H9 test chain (v1.1 activates at H5).

mod v2_capacity_support;
use noid_chain::consensus::forks::{ForkSchedule, V2Activation};
use noid_recursive::acceptance::history_step::v2::V2Config;
use std::path::PathBuf;
use v2_capacity_support::*;

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args
        .first()
        .is_some_and(|s| s == "legacy-tail" || s == "legacy-tail-verify")
    {
        return legacy_tail(&args[1..], args[0] == "legacy-tail-verify");
    }
    if args
        .first()
        .is_some_and(|s| s == "verify" || s == "verify-state")
    {
        return verify_saved(&args[1..], args[0] == "verify-state");
    }
    if !(8..=10).contains(&args.len())
        || args.iter().skip(8).any(|s| {
            s != "--freeze-only"
                && s != "--transition-only"
                && s != "--payments-only"
                && !s.starts_with("--inputs=")
        })
        || args
            .iter()
            .skip(8)
            .filter(|s| s.starts_with("--inputs="))
            .count()
            > 1
        || args
            .iter()
            .skip(8)
            .filter(|s| !s.starts_with("--inputs="))
            .count()
            > 1
    {
        return Err("usage: noid_v2_capacity PACK_ROOT METADATA_PIN LEGACY_FIXTURES NEW_OUTPUT M PAGES SECONDS SAMPLES [--freeze-only|--transition-only|--payments-only] [--inputs=N]".into());
    }
    if noid_chain::consensus::params::V1_1_ACTIVATION_HEIGHT != Some(5) {
        return Err("this fixture requires noid_chain/isolated-v1-1-testnet; the mainnet decoder is intentionally unchanged".into());
    }
    let m = args[4].parse().map_err(err)?;
    let pages = args[5].parse().map_err(err)?;
    let seconds = args[6].parse().map_err(err)?;
    let samples: usize = args[7].parse().map_err(err)?;
    if samples == 0 || samples > 20 {
        return Err("samples must be 1..=20".into());
    }
    let mut config = V2Config::new(
        m,
        pages,
        ForkSchedule::new(Some(5), V2Activation::new(10, seconds))
            .ok_or("invalid fork schedule")?,
    )
    .map_err(err)?;
    if let Some(inputs) = args
        .iter()
        .skip(8)
        .find_map(|s| s.strip_prefix("--inputs="))
    {
        config =
            V2Config::with_input_budget(m, pages, inputs.parse().map_err(err)?, config.schedule())
                .map_err(err)?;
    }
    let pin: [u8; 32] = hex::decode(&args[1])
        .map_err(err)?
        .try_into()
        .map_err(|_| "metadata pin length")?;
    let output = PathBuf::from(&args[3]);
    std::fs::create_dir(&output).map_err(|e| format!("a new output directory is required: {e}"))?;
    let settings = Settings {
        pack: PathBuf::from(&args[0]),
        pin,
        fixtures: PathBuf::from(&args[2]),
        output,
        config,
        samples,
        freeze_only: args.iter().skip(8).any(|s| s == "--freeze-only"),
        transition_only: args.iter().skip(8).any(|s| s == "--transition-only"),
        payments_only: args.iter().skip(8).any(|s| s == "--payments-only"),
    };
    match pages {
        25 => measure::<25>(settings),
        63 => measure::<63>(settings),
        64 => measure::<64>(settings),
        96 => measure::<96>(settings),
        112 => measure::<112>(settings),
        120 => measure::<120>(settings),
        127 => measure::<127>(settings),
        128 => measure::<128>(settings),
        192 => measure::<192>(settings),
        223 => measure::<223>(settings),
        255 => measure::<255>(settings),
        _ => Err("unsupported explicit candidate capacity".into()),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
