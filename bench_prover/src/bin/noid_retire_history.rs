// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Explicit offline release preparation. Verification accepts independent
//! key pins and a metadata-only legacy runtime whose matrix source always
//! fails. Nothing in this tool schedules the public fork or installs a bank.

use noid_ivc_core::{field_r1cs::CompactFieldR1cs, matrix_claim::sparse_c1::*};
use noid_miner::{history_step_artifacts::*, v2_artifacts::*};
use noid_recursive::acceptance::history_step::{self as history, v2::banked as v2};
use noid_recursive::acceptance::history_step_bank::retirement::{
    HistoryStepRetirementReduction, HistoryStepRetirementTarget,
};
use noid_recursive::{
    CanonicalHistoryStepClassId, HistoryStepMatrixLease, HistoryStepMatrixSource,
    HistoryStepMatrixSourceError, HistoryStepRuntime,
};
use serde_json::json;
use std::{
    io::{Read, Write},
    path::Path,
    sync::Arc,
    time::Instant,
};

type Result<T> = std::result::Result<T, String>;
const MATRIX_LIMIT: usize = 1024 * 1024 * 1024;

fn err(error: impl core::fmt::Display) -> String {
    error.to_string()
}
fn pin(text: &str) -> Result<[u8; 32]> {
    hex::decode(text)
        .map_err(err)?
        .try_into()
        .map_err(|_| "pin must be 32 bytes".into())
}
fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err("artifact file bound".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(err)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    if bytes.len() > limit {
        return Err("artifact grew beyond its bound".into());
    }
    Ok(bytes)
}
fn save(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        if read(path, bytes.len())? != bytes {
            return Err(format!("existing artifact differs: {}", path.display()));
        }
        return Ok(());
    }
    let temporary = path.with_extension(format!("{}.partial", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(err)?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(err)?;
    std::fs::rename(temporary, path).map_err(err)?;
    Ok(())
}

struct NoLegacyRows;
impl HistoryStepMatrixSource for NoLegacyRows {
    fn load(
        &self,
        _: CanonicalHistoryStepClassId,
    ) -> std::result::Result<HistoryStepMatrixLease, HistoryStepMatrixSourceError> {
        Err(HistoryStepMatrixSourceError)
    }
}
fn metadata_only(path: &Path, release_pin: [u8; 32]) -> Result<HistoryStepRuntime> {
    let metadata = decode_history_step_runtime_metadata_pinned(
        &read(path, HISTORY_STEP_RUNTIME_METADATA_MAX_BYTES)?,
        release_pin,
    )
    .map_err(err)?;
    let (bank, parts) = metadata.into_parts();
    HistoryStepRuntime::new(bank, Box::new(NoLegacyRows), parts).map_err(err)
}
fn next_bank(path: &Path, release_pin: [u8; 32]) -> Result<v2::Bank> {
    Ok(
        decode_v2_runtime_metadata_pinned(&read(path, V2_METADATA_MAX_BYTES)?, release_pin)?
            .bank()
            .clone(),
    )
}
fn matrix(
    pack: &Path,
    runtime: &HistoryStepRuntime,
    class: CanonicalHistoryStepClassId,
) -> Result<Arc<CompactFieldR1cs>> {
    let entry = runtime.bank().entry(class);
    let compressed = read(
        &pack.join(history_step_matrix_file_name(class)),
        MATRIX_LIMIT,
    )?;
    let mut decoder = zstd::stream::read::Decoder::new(compressed.as_slice()).map_err(err)?;
    decoder.window_log_max(27).map_err(err)?;
    let mut bytes = Vec::new();
    decoder
        .take(MATRIX_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    if bytes.len() > MATRIX_LIMIT {
        return Err("canonical matrix bound".into());
    }
    CompactFieldR1cs::open(
        bytes.into_boxed_slice(),
        entry.shape(),
        entry.matrix_digest(),
    )
    .map(Arc::new)
    .map_err(err)
}

fn prepare(args: &[String]) -> Result<()> {
    if args.len() != 8 && args.len() != 11 {
        return Err("prepare <legacy-pack> <legacy-runtime-pin> <v2-metadata> <v2-bank-pin> <legacy-origin> <output-directory> <max-planned-GiB> [<prepared-key-directory> <independent-key-0-pin> <independent-key-1-pin>]".into());
    }
    let started = Instant::now();
    let pack_directory = Path::new(&args[1]).join(HISTORY_STEP_PACK_VERSION_DIRECTORY);
    let pack = pack_directory.as_path();
    let runtime_pin = pin(&args[2])?;
    let runtime = metadata_only(&pack.join(HISTORY_STEP_RUNTIME_METADATA_FILE), runtime_pin)?;
    // Reuse is explicit release input, never material supplied by the origin.
    // Live classes still rebuild their prover tables from authenticated rows;
    // a previously authenticated inactive key needs no repeated preprocessing.
    let prepared_keys = if args.len() == 11 {
        let directory = Path::new(&args[8]);
        let first = read(&directory.join("class-0.key"), SPARSE_EVALUATION_KEY_BYTES)?;
        let second = read(&directory.join("class-1.key"), SPARSE_EVALUATION_KEY_BYTES)?;
        let pins = [pin(&args[9])?, pin(&args[10])?];
        v2::PinnedRetirementKeys::from_release(runtime.bank(), [&first, &second], pins)
            .map_err(err)?;
        Some([
            SparseMatrixEvaluationKey::from_bytes_pinned(&first, pins[0]).map_err(err)?,
            SparseMatrixEvaluationKey::from_bytes_pinned(&second, pins[1]).map_err(err)?,
        ])
    } else {
        None
    };
    let next = next_bank(Path::new(&args[3]), pin(&args[4])?)?;
    let legacy = v2::LegacyOriginCertificate::from_bytes(&read(
        Path::new(&args[5]),
        v2::MAX_LEGACY_ORIGIN_BYTES,
    )?)
    .map_err(err)?;
    noid_chain::consensus::pow::validate_pow(legacy.parent_header()).map_err(err)?;
    let target =
        HistoryStepRetirementTarget::new(next.config().schedule(), runtime.bank(), next.digest())
            .map_err(err)?;
    let terminal =
        history::decode_history_step_terminal(&runtime, legacy.terminal_bytes()).map_err(err)?;
    let request = history::prepare_history_step_retirement(
        &runtime,
        &terminal,
        legacy.parent_header(),
        legacy.epoch_header(),
        target,
    )
    .map_err(err)?;
    let selected = CanonicalHistoryStepClassId::new(
        noid_chain::history_step::HistoryStepTerminalMetadata::decode_prefix(
            legacy.terminal_bytes(),
        )
        .map_err(err)?
        .class_id() as usize,
    )
    .ok_or("legacy class")?;
    let allowance = args[7].parse::<u64>().map_err(err)?;
    if !(1..=128).contains(&allowance) {
        return Err("planned allowance must be 1..128 GiB; bound worker RSS separately".into());
    }
    let budget = SparseEvaluationBudget {
        max_padded_entries: 1 << 28,
        max_planned_bytes: allowance << 30,
    };
    let output = Path::new(&args[6]);
    std::fs::create_dir_all(output).map_err(err)?;
    let binding = hex::encode(request.binding());
    let identity = json!({"request":binding,"legacy_runtime":hex::encode(runtime_pin),"v2_bank":hex::encode(next.digest())});
    save(
        &output.join("request.json"),
        &serde_json::to_vec_pretty(&identity).map_err(err)?,
    )?;
    println!(
        "{}",
        json!({"stage":"legacy_terminal_replayed","request":binding,"parent_height":legacy.parent_header().height,"selected_class":selected.index()})
    );

    let reduction_path = output.join("reduction.bin");
    let reduction = if reduction_path.exists() {
        HistoryStepRetirementReduction::decode_for(&request, &read(&reduction_path, 8192)?)
            .map_err(err)?
    } else {
        let rows = matrix(pack, &runtime, selected)?;
        let reduction = request
            .prove_reduction(&HistoryStepMatrixLease::Compact(rows))
            .map_err(err)?;
        save(
            &reduction_path,
            &reduction.encode_for(&request).map_err(err)?,
        )?;
        reduction
    };
    let pending = request.verify_reduction(&reduction).map_err(err)?;
    println!(
        "{}",
        json!({"stage":"retirement_reduced","live_classes":pending.obligations().map(|o| o.class_id().index()).collect::<Vec<_>>() })
    );
    let mut key_bytes = Vec::new();
    let mut key_pins = Vec::new();
    let mut evaluations = Vec::new();
    let mut timings = Vec::new();
    for index in 0..2 {
        let class = CanonicalHistoryStepClassId::from_index(index).unwrap();
        let live = pending.obligations().any(|o| o.class_id() == class);
        if !live {
            if let Some(keys) = &prepared_keys {
                let key = &keys[index];
                save(&output.join(format!("class-{index}.key")), &key.to_bytes())?;
                key_bytes.push(key.to_bytes());
                key_pins.push(key.digest());
                timings.push(json!({"class":index,"preprocessing_ms":0,"prove_verify_encode_ms":0,"proof_bytes":0,"reused_inactive_release_key":true}));
                println!(
                    "{}",
                    json!({"stage":"class_complete","result":timings.last()})
                );
                continue;
            }
        }
        let phase = Instant::now();
        println!(
            "{}",
            json!({"stage":"authenticate_preprocess_start","class":index})
        );
        let rows = matrix(pack, &runtime, class)?;
        let prover = SparseMatrixProver::from_compact(
            &rows,
            runtime.bank().entry(class).matrix_digest(),
            budget,
        )
        .map_err(err)?;
        drop(rows);
        let preprocessing_ms = phase.elapsed().as_millis();
        let key = prover.key();
        if prepared_keys
            .as_ref()
            .is_some_and(|keys| keys[index] != *key)
        {
            return Err(
                "recomputed live-class key differs from the independently pinned key".into(),
            );
        }
        save(&output.join(format!("class-{index}.key")), &key.to_bytes())?;
        key_bytes.push(key.to_bytes());
        key_pins.push(key.digest());
        println!(
            "{}",
            json!({"stage":"preprocessing_key_authenticated","class":index,"key_pin":hex::encode(key.digest()),"entries":key.geometry().entries,"padded_entries":key.geometry().padded_entries,"preprocessing_ms":preprocessing_ms})
        );
        let proof_started = Instant::now();
        let mut proof_bytes = 0;
        if let Some(obligation) = pending.obligations().find(|o| o.class_id() == class) {
            let proof_path = output.join(format!("class-{index}.proof"));
            let proof = if proof_path.exists() {
                SparseMatrixEvaluationProof::from_bytes(
                    key,
                    request.binding(),
                    obligation.claim(),
                    &read(&proof_path, MAX_SPARSE_EVALUATION_PROOF_BYTES)?,
                )
                .map_err(err)?
            } else {
                prover
                    .prove_with_disk_workspace(
                        request.binding(),
                        obligation.claim(),
                        budget,
                        output,
                    )
                    .map_err(err)?
            };
            key.verify(request.binding(), obligation.claim(), &proof)
                .map_err(err)?;
            let bytes = proof
                .to_bytes(key, request.binding(), obligation.claim())
                .map_err(err)?;
            save(&proof_path, &bytes)?;
            proof_bytes = bytes.len();
            evaluations.push((class, bytes));
        }
        timings.push(json!({"class":index,"preprocessing_ms":preprocessing_ms,"prove_verify_encode_ms":proof_started.elapsed().as_millis(),"proof_bytes":proof_bytes}));
        println!(
            "{}",
            json!({"stage":"class_complete","result":timings.last()})
        );
        drop(prover);
        noid_ivc_core::scratch::clear();
    }
    let keys = v2::PinnedRetirementKeys::from_release(
        runtime.bank(),
        [&key_bytes[0], &key_bytes[1]],
        [key_pins[0], key_pins[1]],
    )
    .map_err(err)?;
    let certificate = v2::RetirementOriginCertificate::new(
        legacy,
        reduction.encode_for(&request).map_err(err)?,
        evaluations,
    )
    .map_err(err)?;
    let bytes = certificate.to_bytes();
    let decoded = v2::RetirementOriginCertificate::from_bytes(&bytes).map_err(err)?;
    // NoLegacyRows fails every load: this is the real certificate verifier
    // after all large matrices and preprocessing tables have been dropped.
    let verified = decoded.verify(&runtime, &next, &keys).map_err(err)?;
    if verified.origin().boundary() != request.boundary()
        || verified.origin().parent_id() != noid_chain::hash_block_header(request.parent_header())
        || verified.origin().next_bank() != next.digest()
    {
        return Err("retirement origin boundary changed".into());
    }
    save(&output.join("retired-v2-origin.bin"), &bytes)?;
    let report = json!({"status":"passed","retirement_request":binding,"origin_binding":hex::encode(verified.origin().request_binding()),"legacy_runtime":hex::encode(runtime_pin),"legacy_bank":hex::encode(runtime.bank().digest()),"v2_bank":hex::encode(next.digest()),"release_key_pins":key_pins.iter().map(hex::encode).collect::<Vec<_>>(),"certificate_bytes":bytes.len(),"verification_without_legacy_rows":true,"elapsed_ms":started.elapsed().as_millis(),"classes":timings});
    save(
        &output.join("retirement-release.json"),
        &serde_json::to_vec_pretty(&report).map_err(err)?,
    )?;
    println!("{report}");
    Ok(())
}

fn verify(args: &[String]) -> Result<()> {
    if args.len() != 10 && args.len() != 13 {
        return Err("verify <legacy-metadata> <legacy-runtime-pin> <v2-metadata> <v2-bank-pin> <key-0> <pin-0> <key-1> <pin-1> <certificate> [<v2-block> <v2-terminal> <epoch-block|genesis>]".into());
    }
    let runtime = metadata_only(Path::new(&args[1]), pin(&args[2])?)?;
    let next = next_bank(Path::new(&args[3]), pin(&args[4])?)?;
    let first = read(Path::new(&args[5]), SPARSE_EVALUATION_KEY_BYTES)?;
    let second = read(Path::new(&args[7]), SPARSE_EVALUATION_KEY_BYTES)?;
    let keys = v2::PinnedRetirementKeys::from_release(
        runtime.bank(),
        [&first, &second],
        [pin(&args[6])?, pin(&args[8])?],
    )
    .map_err(err)?;
    let certificate_bytes = read(Path::new(&args[9]), v2::MAX_RETIREMENT_ORIGIN_BYTES)?;
    let certificate =
        v2::RetirementOriginCertificate::from_bytes(&certificate_bytes).map_err(err)?;
    let started = Instant::now();
    let verified = certificate.verify(&runtime, &next, &keys).map_err(err)?;
    println!(
        "{}",
        json!({"status":"verified","origin_binding":hex::encode(verified.origin().request_binding()),"verification_without_legacy_rows":true,"verify_ms":started.elapsed().as_millis()})
    );
    if args.len() == 13 {
        let meta_path = Path::new(&args[3]);
        let pack = meta_path.parent().ok_or("v2 pack directory")?;
        let metadata = decode_v2_runtime_metadata_pinned(
            &read(meta_path, V2_METADATA_MAX_BYTES)?,
            pin(&args[4])?,
        )?;
        let new_runtime = metadata.into_runtime([
            Arc::from(read(
                &pack.join(v2_matrix_file_name(v2::Class::Small)),
                V2_MATRIX_MAX_BYTES,
            )?),
            Arc::from(read(
                &pack.join(v2_matrix_file_name(v2::Class::Large)),
                V2_MATRIX_MAX_BYTES,
            )?),
        ])?;
        let header = noid_chain::Block::from_bytes(&read(
            Path::new(&args[10]),
            noid_chain::consensus::wire_limits::MAX_BLOCK_BYTES,
        )?)
        .map_err(|e| format!("block wire: {e:?}"))?
        .header;
        let epoch = if args[12] == "genesis" {
            noid_chain::consensus::genesis_header()
        } else {
            noid_chain::Block::from_bytes(&read(
                Path::new(&args[12]),
                noid_chain::consensus::wire_limits::MAX_BLOCK_BYTES,
            )?)
            .map_err(|e| format!("epoch block wire: {e:?}"))?
            .header
        };
        noid_chain::consensus::pow::validate_pow(&header).map_err(err)?;
        let terminal_bytes = read(
            Path::new(&args[11]),
            noid_chain::consensus::wire_limits::MAX_HISTORY_STEP_TERMINAL_TRANSPORT_BYTES,
        )?;
        let terminal = v2::decode_terminal(&new_runtime, &terminal_bytes).map_err(err)?;
        let start = Instant::now();
        let accepted = v2::verify_terminal(&new_runtime, &verified, &terminal, &header, &epoch)
            .map_err(err)?;
        println!(
            "{}",
            json!({"status":"post_fork_terminal_verified","height":accepted.accumulator().height,
            "class":accepted.class().wire_id(),"verification_without_legacy_rows":true,
            "verify_including_new_matrix_setup_ms":start.elapsed().as_millis()})
        );
        // Exercise the same origin cache and disk lifecycle that the node
        // consumes, with a legacy source that unconditionally refuses rows.
        // The process profile must match the pack's schedule for this check.
        let origin_files = tempfile::tempdir().map_err(err)?;
        let runtime = Arc::new(runtime);
        let new_runtime = Arc::new(new_runtime);
        let make_protocol = || -> Result<noid_miner::HistoryProtocolRuntime> {
            let keys = v2::PinnedRetirementKeys::from_release(
                runtime.bank(),
                [&first, &second],
                [pin(&args[6])?, pin(&args[8])?],
            )
            .map_err(err)?;
            noid_miner::HistoryProtocolRuntime::new(
                Some(Arc::clone(&runtime)),
                Some(Arc::clone(&new_runtime)),
                origin_files.path().to_owned(),
            )?
            .with_legacy_matrix_cache_available(false)
            .with_retirement_keys(keys)
        };
        let protocol = make_protocol()?;
        let requested = protocol
            .requested_origin(&terminal_bytes)?
            .ok_or("missing v2 origin")?;
        if protocol.has_verified_origin(&requested)? || protocol.verified_origin(&requested).is_ok()
        {
            return Err("uncertified origin was accepted".into());
        }
        let installed = protocol.install_origin_bytes(&certificate_bytes)?;
        if installed.origin() != verified.origin() || installed.origin() != &requested {
            return Err("protocol origin binding differs".into());
        }
        protocol.retain_origin(&installed)?;
        protocol.verify_terminal(&terminal_bytes, &header, &epoch)?;
        // A valid old-format transport is insufficient without the old rows.
        if protocol
            .install_legacy_origin(certificate.legacy_certificate().clone())
            .is_ok()
        {
            return Err("legacy-row source unexpectedly available".into());
        }
        drop(protocol);
        let restarted = make_protocol()?;
        if restarted.has_verified_origin(&requested)? {
            return Err("restart retained an unauthenticated in-memory capability".into());
        }
        restarted.verify_terminal(&terminal_bytes, &header, &epoch)?;
        if !restarted.has_verified_origin(&requested)? {
            return Err("disk certificate did not authenticate the origin".into());
        }
        let mut wrong_terminal = terminal_bytes;
        wrong_terminal[9] ^= 1;
        if restarted
            .verify_terminal(&wrong_terminal, &header, &epoch)
            .is_ok()
        {
            return Err("cached origin bypassed terminal verification".into());
        }
        println!(
            "{}",
            json!({"status":"protocol_origin_restart_verified", "height":header.height,
            "verification_without_legacy_rows":true, "missing_certificate_rejected":true,
            "legacy_transport_without_rows_rejected":true, "tampered_terminal_rejected":true})
        );
    }
    Ok(())
}

fn main() {
    noid_ivc_prover::init_perf_thread_pool();
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("prepare") => prepare(&args),
        Some("verify") => verify(&args),
        _ => Err("expected prepare or verify; pins are explicit command inputs".into()),
    };
    if let Err(error) = result {
        eprintln!("retirement failed: {error}");
        std::process::exit(1);
    }
}
