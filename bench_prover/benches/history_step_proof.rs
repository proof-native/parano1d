// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Isolated production HistoryStep proving benchmark.
//!
//! The benchmark requires a completed, pinned v1 pack:
//!
//! ```text
//! source "$PACK_ROOT/pins.env"
//! export NOID_HISTORY_STEP_RUNTIME_METADATA_RELEASE_DIGEST
//! export NOID_HISTORY_STEP_PACK_LEAF_DIGESTS
//! export NOID_HISTORY_STEP_PACK_DIR="$PACK_ROOT"
//! cargo bench -p bench_prover --bench history_step_proof
//! ```
//!
//! Set `NOID_HISTORY_STEP_BENCH_FILTER=B255` (or `c01`) to run B255 after a
//! B25 parent. Use `B255-B255` for the B255-parent case. With no filter, both
//! launch classes run. `NOID_HISTORY_STEP_BENCH_SAMPLES=N` reuses the proved
//! parent and authenticated matrix pack for N production samples, then reports
//! nearest-rank p50/p95 values; the default is one sample.
//! `NOID_HISTORY_STEP_BENCH_WIRE_AUDIT=1` additionally checks exact round trips
//! through both terminal encodings and reports sizes and codec timings outside
//! the timed proving interval. It does not activate any consensus change.
//! `NOID_HISTORY_STEP_BENCH_ALL_PARENTS=1` runs both child classes after each
//! parent class, reusing the authenticated pack. Do not combine it with a filter.
//! Legacy B255 bytes are a diagnostic baseline only: their length exceeds the
//! pre-fork terminal cap, so they are not a pre-fork admissible block.
//!
//! Transaction construction, wallet proving, block-template construction and
//! matrix authentication are setup. `history_step_ms` covers the node's
//! remaining production work: parent-terminal decoding, bounded input and
//! authorization preparation, recursive assembly, nonce sealing, proof
//! construction and terminal encoding. `verify_ms` covers bounded wire decode
//! and complete terminal verification.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, Read as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use bench_prover::{
    HonestHistoryStepFixtureProvider, PreparedHistoryStepBackboneInput,
    PreparedHistoryStepTierFixture,
};
use noid_ivc_core::field_r1cs::CompactFieldR1cs;
use noid_miner::history_step_artifacts::{
    history_step_matrix_file_name, HISTORY_STEP_PACK_LEAF_HASH_DOMAIN,
    HISTORY_STEP_PACK_VERSION_DIRECTORY, HISTORY_STEP_RUNTIME_METADATA_FILE,
    HISTORY_STEP_RUNTIME_METADATA_MAX_BYTES,
};
use noid_poseidon2b::native::poseidon2b_hash_byte_slices;
use noid_recursive::{
    canonical_history_step_shape, decode_history_step_terminal,
    decode_verify_history_step_terminal, encode_history_step_terminal,
    prepare_history_step_for_pow, prove_built_history_step_terminal, prove_history_step,
    CanonicalHistoryStepClassId, ChainAccumulator, HistoryStepBlockInput, HistoryStepMatrixLease,
    HistoryStepMatrixSource, HistoryStepMatrixSourceError, HistoryStepRuntime, HistoryStepTerminal,
    HISTORY_STEP_CLASS_COUNT,
};

const FIXTURE_SEED: u128 = 0x4849_5354_4550_5f56_31;
const PACK_DIRECTORY_ENV: &str = "NOID_HISTORY_STEP_PACK_DIR";
const BENCH_FILTER_ENV: &str = "NOID_HISTORY_STEP_BENCH_FILTER";
const BENCH_SAMPLES_ENV: &str = "NOID_HISTORY_STEP_BENCH_SAMPLES";
const METADATA_DIGEST_ENV: &str = "NOID_HISTORY_STEP_RUNTIME_METADATA_RELEASE_DIGEST";
const LEAF_DIGESTS_ENV: &str = "NOID_HISTORY_STEP_PACK_LEAF_DIGESTS";
const MAX_COMPRESSED_MATRIX_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_CANONICAL_MATRIX_BYTES: usize = 1024 * 1024 * 1024;
const ZSTD_WINDOW_LOG_MAX: u32 = 27;
const MATRIX_CACHE_CAPACITY: usize = 2;

struct PinnedDiskMatrixSource {
    directory: PathBuf,
    matrix_digests: [[u8; 32]; HISTORY_STEP_CLASS_COUNT],
    leaf_digests: [[u8; 32]; HISTORY_STEP_CLASS_COUNT],
    cache: Mutex<VecDeque<(CanonicalHistoryStepClassId, Arc<CompactFieldR1cs>)>>,
}

struct SharedPinnedDiskMatrixSource(Arc<PinnedDiskMatrixSource>);

impl PinnedDiskMatrixSource {
    fn load_checked(
        &self,
        class: CanonicalHistoryStepClassId,
    ) -> Result<Arc<CompactFieldR1cs>, String> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| "HistoryStep benchmark matrix cache is poisoned".to_owned())?;
        if let Some(position) = cache.iter().position(|(cached, _)| *cached == class) {
            let entry = cache
                .remove(position)
                .expect("located benchmark matrix cache entry exists");
            let matrix = Arc::clone(&entry.1);
            cache.push_back(entry);
            return Ok(matrix);
        }
        let path = self.directory.join(history_step_matrix_file_name(class));
        let compressed = read_regular_bounded(&path, MAX_COMPRESSED_MATRIX_BYTES)?;
        let actual =
            poseidon2b_hash_byte_slices(HISTORY_STEP_PACK_LEAF_HASH_DOMAIN, &[&compressed]);
        if actual != self.leaf_digests[class.index()] {
            return Err(format!(
                "compressed leaf pin mismatch for {}",
                path.display()
            ));
        }
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
        let matrix = CompactFieldR1cs::open(
            canonical.into_boxed_slice(),
            canonical_history_step_shape(class),
            self.matrix_digests[class.index()],
        )
        .and_then(CompactFieldR1cs::into_startup_packed)
        .map_err(|error| format!("authenticate {}: {error}", path.display()))?;
        let matrix = Arc::new(matrix);
        cache.push_back((class, Arc::clone(&matrix)));
        while cache.len() > MATRIX_CACHE_CAPACITY {
            cache.pop_front();
        }
        Ok(matrix)
    }
}

impl HistoryStepMatrixSource for SharedPinnedDiskMatrixSource {
    fn load(
        &self,
        class: CanonicalHistoryStepClassId,
    ) -> Result<HistoryStepMatrixLease, HistoryStepMatrixSourceError> {
        self.0
            .load_checked(class)
            .map(HistoryStepMatrixLease::Compact)
            .map_err(|_| HistoryStepMatrixSourceError)
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

fn parse_digest(name: &str) -> Result<[u8; 32], String> {
    let encoded = std::env::var(name).map_err(|_| format!("{name} is required"))?;
    if encoded.len() != 64
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{name} must be 64 lowercase hex characters"));
    }
    let decoded = hex::decode(encoded).map_err(|error| format!("decode {name}: {error}"))?;
    decoded
        .try_into()
        .map_err(|_| format!("{name} must decode to 32 bytes"))
}

fn parse_leaf_digests() -> Result<[[u8; 32]; HISTORY_STEP_CLASS_COUNT], String> {
    let encoded =
        std::env::var(LEAF_DIGESTS_ENV).map_err(|_| format!("{LEAF_DIGESTS_ENV} is required"))?;
    if encoded.len() != HISTORY_STEP_CLASS_COUNT * 64
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{LEAF_DIGESTS_ENV} must contain {HISTORY_STEP_CLASS_COUNT} lowercase hex digests"
        ));
    }
    let mut digests = [[0u8; 32]; HISTORY_STEP_CLASS_COUNT];
    for (index, digest) in digests.iter_mut().enumerate() {
        let start = index * 64;
        let decoded = hex::decode(&encoded[start..start + 64])
            .map_err(|error| format!("decode {LEAF_DIGESTS_ENV} class {index}: {error}"))?;
        digest.copy_from_slice(&decoded);
    }
    Ok(digests)
}

/// Selected case: the current physical-page class plus the parent class the
/// honest fixture chain should build.
fn benchmark_filter() -> Result<Option<(CanonicalHistoryStepClassId, usize)>, String> {
    let value = match std::env::var(BENCH_FILTER_ENV) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(format!("{BENCH_FILTER_ENV} must be valid UTF-8"));
        }
    };
    let class = |slot: usize| CanonicalHistoryStepClassId::new(slot).expect("canonical tier slot");
    let case = match value.trim().to_ascii_lowercase().as_str() {
        "b25" | "25" | "c00" => (class(0), 0),
        "b255" | "255" | "c01" => (class(1), 0),
        "b25-b255" => (class(0), 1),
        "b255-b255" => (class(1), 1),
        _ => {
            return Err(format!(
                "{BENCH_FILTER_ENV} must be one of B25/c00, B255/c01, B25-B255, or B255-B255",
            ));
        }
    };
    Ok(Some(case))
}

fn class_is_selected(
    filter: Option<(CanonicalHistoryStepClassId, usize)>,
    class: CanonicalHistoryStepClassId,
) -> bool {
    filter.map_or(true, |(selected, _)| selected == class)
}

fn benchmark_sample_count() -> Result<usize, String> {
    let value = match std::env::var(BENCH_SAMPLES_ENV) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(1),
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(format!("{BENCH_SAMPLES_ENV} must be valid UTF-8"));
        }
    };
    let samples = value
        .trim()
        .parse::<usize>()
        .map_err(|_| format!("{BENCH_SAMPLES_ENV} must be an integer in 1..=100"))?;
    if !(1..=100).contains(&samples) {
        return Err(format!("{BENCH_SAMPLES_ENV} must be an integer in 1..=100"));
    }
    Ok(samples)
}

fn load_runtime() -> Result<(HistoryStepRuntime, Arc<PinnedDiskMatrixSource>), String> {
    let root = PathBuf::from(
        std::env::var_os(PACK_DIRECTORY_ENV)
            .ok_or_else(|| format!("{PACK_DIRECTORY_ENV} is required"))?,
    );
    let directory = root.join(HISTORY_STEP_PACK_VERSION_DIRECTORY);
    let metadata_path = directory.join(HISTORY_STEP_RUNTIME_METADATA_FILE);
    let encoded = read_regular_bounded(
        &metadata_path,
        HISTORY_STEP_RUNTIME_METADATA_MAX_BYTES as u64,
    )?;
    let metadata = noid_miner::decode_history_step_runtime_metadata_pinned(
        &encoded,
        parse_digest(METADATA_DIGEST_ENV)?,
    )
    .map_err(|error| format!("authenticate {}: {error}", metadata_path.display()))?;
    let matrix_digests =
        std::array::from_fn(|index| metadata.bank().entries()[index].matrix_digest());
    let source = Arc::new(PinnedDiskMatrixSource {
        directory,
        matrix_digests,
        leaf_digests: parse_leaf_digests()?,
        cache: Mutex::new(VecDeque::with_capacity(MATRIX_CACHE_CAPACITY)),
    });
    let (bank, parts) = metadata.into_parts();
    let runtime = HistoryStepRuntime::new(
        bank,
        Box::new(SharedPinnedDiskMatrixSource(Arc::clone(&source))),
        parts,
    )
    .map_err(|error| format!("construct pinned HistoryStep runtime: {error}"))?;
    Ok((runtime, source))
}

fn finish_fixture<const TIER: usize>(
    fixture: PreparedHistoryStepTierFixture<TIER>,
) -> Result<(noid_chain::Block, HistoryStepBlockInput<TIER>), String> {
    let (witness, nonce, start, end) = fixture.into_parts();
    witness
        .finish(nonce, &start, &end)
        .map_err(|error| format!("finish honest B{TIER} fixture: {error}"))
}

fn finish_fixture_template<const TIER: usize>(
    fixture: PreparedHistoryStepTierFixture<TIER>,
) -> Result<(noid_chain::Block, HistoryStepBlockInput<TIER>, u128), String> {
    let (witness, nonce, start, end) = fixture.into_parts();
    let (mut block, input) = witness
        .finish_template(&start, &end)
        .map_err(|error| format!("finish honest B{TIER} template fixture: {error}"))?;
    block.header.nonce = nonce;
    Ok((block, input, nonce))
}

fn prove_parent_step<const TIER: usize>(
    runtime: &HistoryStepRuntime,
    parent: Option<&HistoryStepTerminal>,
    fixture: PreparedHistoryStepTierFixture<TIER>,
) -> Result<(noid_chain::Block, HistoryStepTerminal), String> {
    let (block, input) = finish_fixture(fixture)?;
    let terminal = prove_history_step(runtime, parent, input)
        .map_err(|error| format!("prove honest B{TIER} setup step: {error}"))?;
    Ok((block, terminal))
}

fn build_parent(
    runtime: &HistoryStepRuntime,
    provider: &mut HonestHistoryStepFixtureProvider,
    target_parent_slot: usize,
) -> Result<(noid_chain::Block, HistoryStepTerminal), String> {
    let mut expected = noid_recursive::genesis_accumulator();
    let mut parent = None;
    loop {
        let step = provider.next_backbone(&expected)?.ok_or_else(|| {
            format!("honest backbone ended before parent checkpoint {target_parent_slot}")
        })?;
        let capture = step.capture_parent_slot;
        let (block, terminal) = match step.input {
            PreparedHistoryStepBackboneInput::B25(fixture) => {
                prove_parent_step(runtime, parent.as_ref(), fixture)?
            }
            PreparedHistoryStepBackboneInput::B255(fixture) => {
                prove_parent_step(runtime, parent.as_ref(), fixture)?
            }
        };
        expected = terminal.accumulator().clone();
        if capture == Some(target_parent_slot) {
            return Ok((block, terminal));
        }
        parent = Some(terminal);
    }
}

#[derive(Clone, Copy)]
struct TierMeasurement {
    class_index: usize,
    user_pages: usize,
    wires: usize,
    parent_decode_ms: u128,
    input_preparation_ms: u128,
    staged_assembly_ms: u128,
    prove_encode_ms: u128,
    history_step_ms: u128,
    verify_ms: u128,
    terminal_bytes: usize,
}

fn benchmark_tier<const TIER: usize>(
    runtime: &HistoryStepRuntime,
    parent_bytes: &[u8],
    fixture: PreparedHistoryStepTierFixture<TIER>,
) -> Result<TierMeasurement, String> {
    let input_preparation = fixture.input_preparation();
    let input_preparation_ms = input_preparation.as_millis();
    let user_pages = fixture.user_pages();
    let history_step_started = Instant::now();

    let parent_decode_started = Instant::now();
    let parent = decode_history_step_terminal(runtime, parent_bytes)
        .map_err(|error| format!("decode honest B{TIER} benchmark parent: {error}"))?;
    let parent_decode_ms = parent_decode_started.elapsed().as_millis();

    let (block, input, nonce) = finish_fixture_template(fixture)?;

    let assemble_started = Instant::now();
    let prepared = prepare_history_step_for_pow(runtime, Some(&parent), input)
        .map_err(|error| format!("stage honest B{TIER} benchmark: {error}"))?;
    let staged_assembly_ms = assemble_started.elapsed().as_millis();

    let prove_started = Instant::now();
    let built = prepared
        .seal_nonce(runtime, nonce)
        .map_err(|error| format!("seal honest B{TIER} benchmark: {error}"))?;
    let class = built.class_id();
    let wires = built.useful_rows();
    let terminal = prove_built_history_step_terminal(runtime, &built)
        .map_err(|error| format!("prove honest B{TIER} benchmark: {error}"))?;
    let encoded = encode_history_step_terminal(runtime, &terminal)
        .map_err(|error| format!("encode honest B{TIER} terminal: {error}"))?;
    let prove_encode_ms = prove_started.elapsed().as_millis();
    let history_step_ms = (input_preparation + history_step_started.elapsed()).as_millis();
    let terminal_bytes = encoded.len();
    let epoch_anchor = noid_chain::consensus::genesis_header();
    let verify_started = Instant::now();
    let accepted =
        decode_verify_history_step_terminal(runtime, &encoded, &block.header, &epoch_anchor)
            .map_err(|error| format!("verify honest B{TIER} terminal: {error}"))?;
    let verify_ms = verify_started.elapsed().as_millis();
    if accepted.class_id() != class || accepted.semantic_id() != terminal.semantic_id() {
        return Err(format!("B{TIER} accepted terminal boundary drift"));
    }
    if std::env::var_os("NOID_HISTORY_STEP_BENCH_WIRE_AUDIT").is_some() {
        let audit =
            noid_recursive::acceptance::history_step::audit_history_step_terminal_encodings(
                runtime,
                &terminal,
                &block.header,
                &epoch_anchor,
            )
            .map_err(|error| format!("B{TIER} wire roundtrip audit: {error}"))?;
        println!("B{TIER} wire_audit legacy_bytes={} shared_bytes={} saved_bytes={} exact_legacy_roundtrip=true original_terminal_verified=true both_formats_native_verified=true fork_format_checks={} malformed_inputs_rejected={}", audit.legacy_bytes, audit.shared_bytes, audit.legacy_bytes - audit.shared_bytes, audit.fork_format_checks, audit.malformed_inputs_rejected);
        for (operation, values) in [
            ("legacy_encode", &audit.legacy_encode_ns),
            ("shared_encode", &audit.shared_encode_ns),
            ("legacy_decode", &audit.legacy_decode_ns),
            ("shared_decode", &audit.shared_decode_ns),
        ] {
            println!(
                "B{TIER} wire_codec operation={operation} samples={} p50_us={:.3} p95_us={:.3}",
                values.len(),
                nearest_rank(values.clone(), 50) as f64 / 1000.0,
                nearest_rank(values.clone(), 95) as f64 / 1000.0,
            );
        }
    }

    Ok(TierMeasurement {
        class_index: class.index(),
        user_pages,
        wires,
        parent_decode_ms,
        input_preparation_ms,
        staged_assembly_ms,
        prove_encode_ms,
        history_step_ms,
        verify_ms,
        terminal_bytes,
    })
}

fn nearest_rank(mut values: Vec<u128>, percentile: usize) -> u128 {
    assert!(!values.is_empty());
    assert!((1..=100).contains(&percentile));
    values.sort_unstable();
    let rank = (percentile * values.len()).div_ceil(100);
    values[rank.saturating_sub(1)]
}

fn report_samples<const TIER: usize>(measurements: &[TierMeasurement]) {
    if measurements.len() == 1 {
        return;
    }

    let metric = |select: fn(TierMeasurement) -> u128| {
        let values: Vec<_> = measurements.iter().copied().map(select).collect();
        (nearest_rank(values.clone(), 50), nearest_rank(values, 95))
    };
    let (assembly_p50, assembly_p95) = metric(|sample| sample.staged_assembly_ms);
    let (parent_decode_p50, parent_decode_p95) = metric(|sample| sample.parent_decode_ms);
    let (input_p50, input_p95) = metric(|sample| sample.input_preparation_ms);
    let (prove_p50, prove_p95) = metric(|sample| sample.prove_encode_ms);
    let (history_step_p50, history_step_p95) = metric(|sample| sample.history_step_ms);
    let (verify_p50, verify_p95) = metric(|sample| sample.verify_ms);
    println!(
        "B{TIER} summary samples={} user_pages={} wires={} parent_decode_p50_ms={parent_decode_p50} parent_decode_p95_ms={parent_decode_p95} input_preparation_p50_ms={input_p50} input_preparation_p95_ms={input_p95} staged_assembly_p50_ms={assembly_p50} staged_assembly_p95_ms={assembly_p95} prove_encode_p50_ms={prove_p50} prove_encode_p95_ms={prove_p95} history_step_p50_ms={history_step_p50} history_step_p95_ms={history_step_p95} verify_p50_ms={verify_p50} verify_p95_ms={verify_p95} terminal_bytes={}",
        measurements.len(),
        measurements[0].user_pages,
        measurements[0].wires,
        measurements[0].terminal_bytes,
    );
}

fn benchmark_tier_samples<const TIER: usize>(
    runtime: &HistoryStepRuntime,
    parent: &HistoryStepTerminal,
    sample_count: usize,
    mut fixture: impl FnMut() -> Result<PreparedHistoryStepTierFixture<TIER>, String>,
) -> Result<(), String> {
    let parent_bytes = encode_history_step_terminal(runtime, parent)
        .map_err(|error| format!("encode B{TIER} benchmark parent: {error}"))?;
    let mut measurements = Vec::with_capacity(sample_count);
    for sample in 0..sample_count {
        let measurement = benchmark_tier(runtime, &parent_bytes, fixture()?)?;
        println!(
            "B{TIER} class=c{:02} sample={}/{} user_pages={} wires={} parent_decode_ms={} input_preparation_ms={} staged_assembly_ms={} prove_encode_ms={} history_step_ms={} verify_ms={} terminal_bytes={}",
            measurement.class_index,
            sample + 1,
            sample_count,
            measurement.user_pages,
            measurement.wires,
            measurement.parent_decode_ms,
            measurement.input_preparation_ms,
            measurement.staged_assembly_ms,
            measurement.prove_encode_ms,
            measurement.history_step_ms,
            measurement.verify_ms,
            measurement.terminal_bytes,
        );
        measurements.push(measurement);
    }
    let expected = measurements[0];
    if measurements.iter().any(|measurement| {
        measurement.class_index != expected.class_index
            || measurement.user_pages != expected.user_pages
            || measurement.wires != expected.wires
            || measurement.terminal_bytes != expected.terminal_bytes
    }) {
        return Err(format!("B{TIER} repeated benchmark shape drift"));
    }
    report_samples::<TIER>(&measurements);
    Ok(())
}

fn run_on_production_pool() -> Result<(), String> {
    let filter = benchmark_filter()?;
    let sample_count = benchmark_sample_count()?;
    let (runtime, source) = load_runtime()?;
    let all_parents = std::env::var_os("NOID_HISTORY_STEP_BENCH_ALL_PARENTS").is_some();
    if all_parents && filter.is_some() {
        return Err("ALL_PARENTS cannot be combined with a class filter".to_owned());
    }
    let parent_slots = if all_parents {
        vec![0, 1]
    } else {
        vec![filter.map_or(0, |(_, parent_slot)| parent_slot)]
    };
    let c00 = CanonicalHistoryStepClassId::new(0).expect("B25 class");
    for target_parent_slot in parent_slots {
        println!("HistoryStep benchmark parent_class=c{target_parent_slot:02}");
        let mut provider = HonestHistoryStepFixtureProvider::new(FIXTURE_SEED)?;
        // Authenticate and convert outside every reported assembly interval. The
        // production node receives the same packed layout from its release build;
        // this file-backed benchmark deliberately performs the full canonical
        // authentication itself instead of minting a file-derived build seal.
        source.load_checked(c00)?;
        let (parent_block, parent) = build_parent(&runtime, &mut provider, target_parent_slot)?;

        let parent_wire = encode_history_step_terminal(&runtime, &parent)
            .map_err(|error| format!("encode benchmark parent: {error}"))?;
        let accepted_parent = decode_verify_history_step_terminal(
            &runtime,
            &parent_wire,
            &parent_block.header,
            &noid_chain::consensus::genesis_header(),
        )
        .map_err(|error| format!("verify benchmark parent: {error}"))?;
        if accepted_parent.class_id() != parent.class_id()
            || accepted_parent.semantic_id() != parent.semantic_id()
        {
            return Err("verified benchmark parent boundary drift".to_owned());
        }
        if provider.parent_accumulator(target_parent_slot) != Some(parent.accumulator()) {
            return Err("benchmark checkpoint and proved parent disagree".to_owned());
        }

        let parent_accumulator: &ChainAccumulator = parent.accumulator();
        let parent_class = parent.class_id();
        // The production LRU holds exactly two matrices. Preload current first
        // and the exact parent class second before every timed recursive build;
        // this remains correct when the unfiltered benchmark advances to B255 and
        // would otherwise evict the parent while admitting the current class.
        if class_is_selected(filter, c00) {
            source.load_checked(c00)?;
            source.load_checked(parent_class)?;
            benchmark_tier_samples(&runtime, &parent, sample_count, || {
                provider.b25_coinbase_only(c00, parent_accumulator)
            })?;
        }
        let c01 = CanonicalHistoryStepClassId::new(1).expect("B255 class");
        if class_is_selected(filter, c01) {
            source.load_checked(c01)?;
            source.load_checked(parent_class)?;
            benchmark_tier_samples(&runtime, &parent, sample_count, || {
                provider.b255(c01, parent_accumulator)
            })?;
        }
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let cpu_plan =
        noid_miner::configure_process_cpu_budget(noid_miner::ProcessCpuBudgetMode::ProofOnly)
            .map_err(|error| format!("configure production CPU pool: {error}"))?;
    let hardware = noid_core::cpu::ProductionHardwareReport::detect();
    println!(
        "HistoryStep benchmark backend={} threads={}",
        hardware.backend, cpu_plan.history_step_phase_threads,
    );
    noid_miner::install_history_step_phase_cpu(run_on_production_pool)
        .map_err(|error| format!("enter production HistoryStep CPU phase: {error}"))?
}

fn main() {
    if let Err(error) = run() {
        panic!("HistoryStep production benchmark: {error}");
    }
}
