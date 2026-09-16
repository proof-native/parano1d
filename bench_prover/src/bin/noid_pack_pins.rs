// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Emit release pins for one canonical `HistoryStep` v1 pack.
//!
//! Usage: `noid_pack_pins <pack-root>`

use std::fs::File;
use std::io::{BufReader, Read as _};
use std::path::Path;
use std::sync::Arc;

use bench_prover::{
    HonestHistoryStepFixtureProvider, PreparedHistoryStepBackboneInput, V2ResearchMutation,
};
use noid_ivc_core::field_r1cs::CompactFieldR1cs;
use noid_miner::history_step_artifacts::{
    decode_history_step_runtime_metadata_pinned, history_step_matrix_file_name,
    HISTORY_STEP_PACK_LEAF_COUNT, HISTORY_STEP_PACK_LEAF_HASH_DOMAIN,
    HISTORY_STEP_PACK_VERSION_DIRECTORY, HISTORY_STEP_RUNTIME_METADATA_FILE,
    HISTORY_STEP_RUNTIME_METADATA_MAX_BYTES,
};
use noid_poseidon2b::native::poseidon2b_hash_byte_slices;
use noid_recursive::{
    acceptance::history_step::{
        assemble_frozen_direct_block_research, assemble_frozen_history_step_base,
        assemble_history_step_base,
    },
    canonical_history_step_shape, CanonicalHistoryStepClassId, HistoryStepMatrixLease,
    HistoryStepMatrixSource, HistoryStepMatrixSourceError, HistoryStepRuntime,
};

const FIXTURE_SEED: u128 = 0x4849_5354_4550_5f56_31;
const MAX_COMPRESSED_MATRIX_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_CANONICAL_MATRIX_BYTES: usize = 1024 * 1024 * 1024;
const ZSTD_WINDOW_LOG_MAX: u32 = 27;

struct LaunchMatrixSource {
    class: CanonicalHistoryStepClassId,
    matrix: Arc<CompactFieldR1cs>,
}

impl HistoryStepMatrixSource for LaunchMatrixSource {
    fn load(
        &self,
        class: CanonicalHistoryStepClassId,
    ) -> Result<HistoryStepMatrixLease, HistoryStepMatrixSourceError> {
        if class != self.class {
            return Err(HistoryStepMatrixSourceError);
        }
        Ok(HistoryStepMatrixLease::Compact(Arc::clone(&self.matrix)))
    }
}

fn read_regular_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("inspect {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > max_bytes {
        return Err(format!("{} is not a bounded regular file", path.display()));
    }
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| format!("{} length does not fit usize", path.display()))?;
    let mut bytes = Vec::with_capacity(capacity);
    File::open(path)
        .map_err(|error| format!("open {}: {error}", path.display()))?
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!("{} exceeds its byte bound", path.display()));
    }
    Ok(bytes)
}

fn open_launch_matrix(
    path: &Path,
    class: CanonicalHistoryStepClassId,
    expected_digest: [u8; 32],
) -> Result<CompactFieldR1cs, String> {
    let compressed = read_regular_bounded(path, MAX_COMPRESSED_MATRIX_BYTES)?;
    let mut decoder = zstd::stream::read::Decoder::new(BufReader::new(compressed.as_slice()))
        .map_err(|error| format!("open zstd {}: {error}", path.display()))?;
    decoder
        .window_log_max(ZSTD_WINDOW_LOG_MAX)
        .map_err(|error| format!("bound zstd window {}: {error}", path.display()))?;
    let mut canonical = Vec::new();
    decoder
        .take((MAX_CANONICAL_MATRIX_BYTES + 1) as u64)
        .read_to_end(&mut canonical)
        .map_err(|error| format!("decode {}: {error}", path.display()))?;
    if canonical.len() > MAX_CANONICAL_MATRIX_BYTES {
        return Err(format!(
            "{} exceeds the canonical size bound",
            path.display()
        ));
    }
    CompactFieldR1cs::open(
        canonical.into_boxed_slice(),
        canonical_history_step_shape(class),
        expected_digest,
    )
    .and_then(CompactFieldR1cs::into_startup_packed)
    .map_err(|error| format!("authenticate {}: {error}", path.display()))
}

fn validate_launch_compatibility(
    runtime: &HistoryStepRuntime,
) -> Result<(CanonicalHistoryStepClassId, usize), String> {
    let mut provider = HonestHistoryStepFixtureProvider::new(FIXTURE_SEED)?;
    let genesis = noid_recursive::genesis_accumulator();
    let step = provider
        .next_backbone(&genesis)?
        .ok_or_else(|| "honest HistoryStep launch fixture is missing".to_owned())?;
    let PreparedHistoryStepBackboneInput::B25(prepared) = step.input else {
        return Err("honest HistoryStep launch fixture is not B25".to_owned());
    };
    let (witness, nonce, start, end) = prepared.into_parts();
    let (_, input) = witness
        .finish(nonce, &start, &end)
        .map_err(|error| format!("finish honest HistoryStep launch fixture: {error}"))?;
    let built = assemble_history_step_base(runtime, input)
        .map_err(|error| format!("assemble current HistoryStep launch witness: {error}"))?;
    Ok((built.class_id(), built.useful_rows()))
}

fn measure_v2_full_history_step(
    runtime: &HistoryStepRuntime,
) -> Result<(CanonicalHistoryStepClassId, usize, [u8; 32], bool), String> {
    let mut provider = HonestHistoryStepFixtureProvider::new(FIXTURE_SEED)?;
    let genesis = noid_recursive::genesis_accumulator();
    let step = provider
        .next_backbone(&genesis)?
        .ok_or_else(|| "honest HistoryStep launch fixture is missing".to_owned())?;
    let PreparedHistoryStepBackboneInput::B25(prepared) = step.input else {
        return Err("honest HistoryStep launch fixture is not B25".to_owned());
    };
    let (witness, nonce, start, end) = prepared.into_parts();
    let (_, input) = witness
        .finish(nonce, &start, &end)
        .map_err(|error| format!("finish honest HistoryStep launch fixture: {error}"))?;
    let built = assemble_frozen_history_step_base(runtime, input)
        .map_err(|error| format!("assemble research HistoryStep launch witness: {error}"))?;
    let digest = built.matrix().statement_digest();
    let satisfied = built.matrix().satisfies(built.witness());
    Ok((built.class_id(), built.useful_rows(), digest, satisfied))
}

fn measure_v2_heterogeneous_direct_blocks() -> Result<(), String> {
    let mut provider = HonestHistoryStepFixtureProvider::new(FIXTURE_SEED)?;
    let mut expected = noid_recursive::genesis_accumulator();
    loop {
        let step = provider
            .next_backbone(&expected)?
            .ok_or_else(|| "honest backbone ended before B255 checkpoint".to_owned())?;
        let captured = step.capture_parent_slot == Some(1);
        expected = match &step.input {
            PreparedHistoryStepBackboneInput::B25(input) => input.end_accumulator().clone(),
            PreparedHistoryStepBackboneInput::B255(input) => input.end_accumulator().clone(),
        };
        if captured {
            break;
        }
    }

    let mut reference_digest = None;
    let mut reference_rows = None;
    for contract_count in [0usize, 1, 4, noid_recursive::V2_CONTRACT_SLOTS] {
        let started = std::time::Instant::now();
        let input = provider.v2_research_b25_input(contract_count)?;
        let prepared_ms = started.elapsed().as_secs_f64() * 1e3;
        let build_started = std::time::Instant::now();
        let frozen = assemble_frozen_direct_block_research(input)
            .map_err(|error| format!("assemble {contract_count}-call direct block: {error}"))?;
        let build_ms = build_started.elapsed().as_secs_f64() * 1e3;
        let digest = frozen.matrix().statement_digest();
        let satisfied = frozen.matrix().satisfies(frozen.witness());
        let rows = frozen.useful_rows();
        if let Some(reference) = reference_digest {
            if digest != reference {
                return Err(format!(
                    "{contract_count}-call direct matrix differs from zero-call matrix"
                ));
            }
        } else {
            reference_digest = Some(digest);
        }
        if let Some(reference) = reference_rows {
            if rows != reference {
                return Err(format!(
                    "{contract_count}-call direct row count {rows} differs from {reference}"
                ));
            }
        } else {
            reference_rows = Some(rows);
        }
        if !satisfied {
            return Err(format!(
                "{contract_count}-call direct witness is unsatisfied"
            ));
        }
        println!(
            "v2-direct contracts={contract_count} pages=25 rows={rows} digest={} satisfied={satisfied} prepare_ms={prepared_ms:.1} build_and_scan_ms={build_ms:.1}",
            hex::encode(digest),
        );
    }

    let reference_digest = reference_digest.ok_or_else(|| "missing direct digest".to_owned())?;
    let reference_rows = reference_rows.ok_or_else(|| "missing direct row count".to_owned())?;
    for (label, contract_count, mutation) in [
        (
            "invalid-opcode-opening",
            1usize,
            V2ResearchMutation::InvalidOpcodeOpening,
        ),
        (
            "wrong-next-opening",
            1usize,
            V2ResearchMutation::WrongNextOpening,
        ),
        (
            "wrong-context-opening",
            1usize,
            V2ResearchMutation::WrongContextOpening,
        ),
        (
            "wrong-terminal-recipient",
            2usize,
            V2ResearchMutation::WrongTerminalRecipient,
        ),
    ] {
        let input = provider.v2_research_b25_input_with_mutation(contract_count, mutation)?;
        let frozen = assemble_frozen_direct_block_research(input)
            .map_err(|error| format!("assemble negative case {label}: {error}"))?;
        let digest = frozen.matrix().statement_digest();
        let rows = frozen.useful_rows();
        let satisfied = frozen.matrix().satisfies(frozen.witness());
        if digest != reference_digest || rows != reference_rows {
            return Err(format!(
                "negative case {label} changed fixed shape: rows={rows}, digest={}",
                hex::encode(digest)
            ));
        }
        if satisfied {
            return Err(format!("negative case {label} unexpectedly satisfied"));
        }
        println!(
            "v2-negative case={label} contracts={contract_count} rows={rows} digest={} satisfied={satisfied}",
            hex::encode(digest),
        );
    }

    match provider
        .v2_research_b25_input_with_mutation(2, V2ResearchMutation::WrongBranchAuthorization)
    {
        Ok(_) => return Err("wrong-branch authorization unexpectedly prepared".to_owned()),
        Err(error) => println!(
            "v2-negative case=wrong-branch-authorization native_rejected=true error={error}"
        ),
    }

    let mut reference_digest = None;
    let mut reference_rows = None;
    for contract_count in [0usize, 1, 4, noid_recursive::V2_CONTRACT_SLOTS] {
        let started = std::time::Instant::now();
        let input = provider.v2_research_b255_input(contract_count)?;
        let prepared_ms = started.elapsed().as_secs_f64() * 1e3;
        let build_started = std::time::Instant::now();
        let frozen = assemble_frozen_direct_block_research(input).map_err(|error| {
            format!("assemble B255 {contract_count}-call direct block: {error}")
        })?;
        let build_ms = build_started.elapsed().as_secs_f64() * 1e3;
        let digest = frozen.matrix().statement_digest();
        let satisfied = frozen.matrix().satisfies(frozen.witness());
        let rows = frozen.useful_rows();
        if let Some(reference) = reference_digest {
            if digest != reference {
                return Err(format!(
                    "B255 {contract_count}-call direct matrix differs from zero-call matrix"
                ));
            }
        } else {
            reference_digest = Some(digest);
        }
        if let Some(reference) = reference_rows {
            if rows != reference {
                return Err(format!(
                    "B255 {contract_count}-call direct row count {rows} differs from {reference}"
                ));
            }
        } else {
            reference_rows = Some(rows);
        }
        if !satisfied {
            return Err(format!(
                "B255 {contract_count}-call direct witness is unsatisfied"
            ));
        }
        println!(
            "v2-direct tier=255 contracts={contract_count} pages=26 rows={rows} digest={} satisfied={satisfied} prepare_ms={prepared_ms:.1} build_and_scan_ms={build_ms:.1}",
            hex::encode(digest),
        );
    }
    Ok(())
}

fn main() {
    noid_ivc_prover::init_perf_thread_pool();
    if std::env::var_os("NOID_V2_DIRECT_ONLY").is_some() {
        measure_v2_heterogeneous_direct_blocks()
            .unwrap_or_else(|error| panic!("v2 heterogeneous direct-block measurement: {error}"));
        return;
    }
    let root = std::env::args()
        .nth(1)
        .expect("usage: noid_pack_pins <pack-root>");
    let root = std::path::Path::new(&root);
    let version = root.join(HISTORY_STEP_PACK_VERSION_DIRECTORY);

    let metadata_path = version.join(HISTORY_STEP_RUNTIME_METADATA_FILE);
    let metadata = read_regular_bounded(
        &metadata_path,
        HISTORY_STEP_RUNTIME_METADATA_MAX_BYTES as u64,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let trailer_start = metadata
        .len()
        .checked_sub(32)
        .expect("runtime metadata is shorter than its digest trailer");
    let metadata_digest: [u8; 32] = metadata[trailer_start..]
        .try_into()
        .expect("fixed metadata trailer");
    let runtime_metadata = decode_history_step_runtime_metadata_pinned(&metadata, metadata_digest)
        .expect("canonical HistoryStep runtime metadata");
    println!(
        "{}  {}",
        hex::encode(metadata_digest),
        metadata_path
            .file_name()
            .expect("metadata file name")
            .to_string_lossy()
    );

    let mut leaf_pins = String::with_capacity(HISTORY_STEP_PACK_LEAF_COUNT * 64);
    let launch_class = CanonicalHistoryStepClassId::new(0).expect("canonical launch class");
    let launch_path = version.join(history_step_matrix_file_name(launch_class));
    for index in 0..HISTORY_STEP_PACK_LEAF_COUNT {
        let class = CanonicalHistoryStepClassId::from_index(index).expect("canonical class");
        let leaf_path = version.join(history_step_matrix_file_name(class));
        let bytes = read_regular_bounded(&leaf_path, MAX_COMPRESSED_MATRIX_BYTES)
            .unwrap_or_else(|error| panic!("{error}"));
        let digest = poseidon2b_hash_byte_slices(HISTORY_STEP_PACK_LEAF_HASH_DOMAIN, &[&bytes]);
        let encoded = hex::encode(digest);
        println!(
            "{encoded}  {}  ({:.1} MiB)",
            leaf_path
                .file_name()
                .expect("matrix file name")
                .to_string_lossy(),
            bytes.len() as f64 / (1024.0 * 1024.0)
        );
        leaf_pins.push_str(&encoded);
    }

    let launch_digest = runtime_metadata.bank().entry(launch_class).matrix_digest();
    let launch_matrix = open_launch_matrix(&launch_path, launch_class, launch_digest)
        .unwrap_or_else(|error| panic!("{error}"));
    let (bank, runtime_parts) = runtime_metadata.into_parts();
    let runtime = HistoryStepRuntime::new(
        bank,
        Box::new(LaunchMatrixSource {
            class: launch_class,
            matrix: Arc::new(launch_matrix),
        }),
        runtime_parts,
    )
    .expect("construct canonical HistoryStep runtime");
    if std::env::var_os("NOID_V2_FULL_ONLY").is_some() {
        let (class_id, rows, digest, satisfied) = measure_v2_full_history_step(&runtime)
            .unwrap_or_else(|error| panic!("v2 full HistoryStep measurement: {error}"));
        println!(
            "research frozen launch matrix: c{:02}, {rows} useful rows, digest={}, satisfied={satisfied}",
            class_id.index(),
            hex::encode(digest),
        );
        return;
    }
    let (validated_class, useful_rows) = validate_launch_compatibility(&runtime)
        .unwrap_or_else(|error| panic!("HistoryStep pack/source incompatibility: {error}"));
    assert_eq!(
        validated_class, launch_class,
        "launch witness selected an unexpected HistoryStep class"
    );
    println!(
        "current source launch compatibility: c{:02}, {useful_rows} useful rows",
        validated_class.index()
    );

    println!(
        "\nNOID_HISTORY_STEP_RUNTIME_METADATA_RELEASE_DIGEST={}",
        hex::encode(metadata_digest)
    );
    println!("NOID_HISTORY_STEP_PACK_LEAF_DIGESTS={leaf_pins}");
}
