use super::*;
use noid_ivc_core::field_r1cs::{CompactFieldR1cs, FieldR1cs};
use noid_miner::history_step_artifacts::*;
use noid_recursive::{
    canonical_history_step_shape, CanonicalHistoryStepClassId, HistoryStepMatrixLease,
    HistoryStepMatrixSource, HistoryStepMatrixSourceError, HistoryStepRuntime, HistoryStepTerminal,
};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

const MAX_MATRIX: usize = 1024 * 1024 * 1024;

pub fn bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let meta = std::fs::symlink_metadata(path).map_err(err)?;
    if !meta.is_file() || meta.len() > limit as u64 {
        return Err("artifact file bound".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(err)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    if bytes.len() > limit {
        return Err("artifact grew beyond bound".into());
    }
    Ok(bytes)
}

fn open_matrix(
    path: &Path,
    shape: noid_ivc_core::proof::FieldShape,
    digest: [u8; 32],
) -> Result<Arc<CompactFieldR1cs>> {
    let compressed = bounded(path, MAX_MATRIX)?;
    let mut decoder = zstd::stream::read::Decoder::new(compressed.as_slice()).map_err(err)?;
    decoder.window_log_max(27).map_err(err)?;
    let mut bytes = Vec::new();
    decoder
        .take(MAX_MATRIX as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    if bytes.len() > MAX_MATRIX {
        return Err("canonical matrix bound".into());
    }
    Ok(Arc::new(
        CompactFieldR1cs::open(bytes.into_boxed_slice(), shape, digest)
            .and_then(CompactFieldR1cs::into_startup_packed)
            .map_err(err)?,
    ))
}

struct LegacySource {
    root: PathBuf,
    digests: [[u8; 32]; 2],
    cache: Mutex<[Option<Arc<CompactFieldR1cs>>; 2]>,
}
impl HistoryStepMatrixSource for LegacySource {
    fn load(
        &self,
        class: CanonicalHistoryStepClassId,
    ) -> std::result::Result<HistoryStepMatrixLease, HistoryStepMatrixSourceError> {
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| HistoryStepMatrixSourceError)?;
        let i = class.index();
        if cache[i].is_none() {
            cache[i] = Some(
                open_matrix(
                    &self.root.join(history_step_matrix_file_name(class)),
                    canonical_history_step_shape(class),
                    self.digests[i],
                )
                .map_err(|_| HistoryStepMatrixSourceError)?,
            );
        }
        Ok(HistoryStepMatrixLease::Compact(Arc::clone(
            cache[i].as_ref().unwrap(),
        )))
    }
}

struct SingleSource(Option<Arc<CompactFieldR1cs>>);
impl v2::V2MatrixSource for SingleSource {
    fn load(&self) -> std::result::Result<HistoryStepMatrixLease, v2::V2Error> {
        self.0
            .as_ref()
            .map(|m| HistoryStepMatrixLease::Compact(Arc::clone(m)))
            .ok_or(v2::V2Error::Matrix)
    }
}

pub fn open_candidate(output: &Path, expected_bank: [u8; 32]) -> Result<v2::V2Runtime> {
    let parts = v2::V2RuntimeParts::decode_compact(&bounded(
        &output.join("candidate.parts"),
        noid_recursive::HISTORY_STEP_RUNTIME_PARTS_COMPACT_MAX_BYTES,
    )?)
    .map_err(err)?;
    let pins: serde_json::Value =
        serde_json::from_slice(&bounded(&output.join("candidate.pins.json"), 16 * 1024)?)
            .map_err(err)?;
    let matrix_digest: [u8; 32] = hex::decode(pins["matrix"].as_str().ok_or("missing matrix pin")?)
        .map_err(err)?
        .try_into()
        .map_err(|_| "matrix pin length")?;
    let bank = v2::V2Bank::pin(matrix_digest, &parts);
    if bank.digest() != expected_bank {
        return Err("candidate bank differs from supplied pin".into());
    }
    let matrix = open_matrix(
        &output.join("candidate.field-r1cs.zst"),
        parts.config().shape(),
        matrix_digest,
    )?;
    v2::V2Runtime::new(bank, parts, Box::new(SingleSource(Some(matrix)))).map_err(err)
}

pub fn legacy_runtime(pack: &Path, pin: [u8; 32]) -> Result<HistoryStepRuntime> {
    let root = pack.join(HISTORY_STEP_PACK_VERSION_DIRECTORY);
    let bytes = bounded(
        &root.join(HISTORY_STEP_RUNTIME_METADATA_FILE),
        HISTORY_STEP_RUNTIME_METADATA_MAX_BYTES,
    )?;
    let metadata = decode_history_step_runtime_metadata_pinned(&bytes, pin).map_err(err)?;
    let (bank, parts) = metadata.into_parts();
    let digests = std::array::from_fn(|i| bank.entries()[i].matrix_digest());
    HistoryStepRuntime::new(
        bank,
        Box::new(LegacySource {
            root,
            digests,
            cache: Mutex::new([None, None]),
        }),
        parts,
    )
    .map_err(err)
}

pub fn freeze<const PAGES: usize>(
    settings: &Settings,
    legacy: &HistoryStepRuntime,
    legacy_tip: &HistoryStepTerminal,
    parent: &BlockHeader,
    epoch: &BlockHeader,
    mut make: impl FnMut() -> Result<noid_recursive::HistoryStepBlockInput<PAGES>>,
) -> Result<(v2::V2Runtime, v2::VerifiedV2Origin)> {
    let start = Instant::now();
    let config = settings.config;
    let mut parts = v2::derive_runtime_parts(
        config,
        v2::derive_direct_block_vk(config, make()?).map_err(err)?,
    )
    .map_err(err)?;
    let mut candidate = None;
    for pass in 0..4 {
        let runtime = v2::V2Runtime::new(
            v2::V2Bank::pin([0; 32], &parts),
            parts.clone(),
            Box::new(SingleSource(None)),
        )
        .map_err(err)?;
        let origin =
            v2::VerifiedV2Origin::from_legacy(legacy, legacy_tip, parent, epoch, runtime.bank())
                .map_err(err)?;
        let scan = Instant::now();
        let frozen = v2::assemble_frozen(&runtime, origin.origin(), None, make()?).map_err(err)?;
        if !frozen.matrix().satisfies(frozen.witness()) {
            return Err("complete candidate matrix has an unsatisfied witness".into());
        }
        if frozen.parent_vk() != parts.parent_vk() {
            return Err("parent VK layout drift".into());
        }
        println!(
            "{}",
            json!({"phase":"freeze", "pass":pass, "m":config.outer_m(),
            "pages":PAGES, "rows":frozen.matrix().useful_rows, "satisfied":true,
            "matrix":hex::encode(frozen.matrix().statement_digest()),
            "build_scan_ms":elapsed(scan), "memory":memory()})
        );
        if frozen.block_vk() == parts.block_vk() {
            candidate = Some(frozen);
            break;
        }
        parts = v2::derive_runtime_parts(config, frozen.block_vk().clone()).map_err(err)?;
    }
    let candidate = candidate.ok_or("integrated registry slices failed to converge")?;
    let rows = candidate.matrix().useful_rows;
    let bank = v2::V2Bank::pin(candidate.matrix().statement_digest(), &parts);
    let matrix_path = settings.output.join("candidate.field-r1cs.zst");
    write_matrix(&matrix_path, candidate.matrix())?;
    drop(candidate);
    let matrix = open_matrix(&matrix_path, config.shape(), bank.matrix_digest())?;
    let runtime = v2::V2Runtime::new(bank, parts.clone(), Box::new(SingleSource(Some(matrix))))
        .map_err(err)?;
    let origin =
        v2::VerifiedV2Origin::from_legacy(legacy, legacy_tip, parent, epoch, runtime.bank())
            .map_err(err)?;
    // Final identities are public inputs, never witness-dependent coefficients.
    let final_matrix =
        v2::assemble_frozen(&runtime, origin.origin(), None, make()?).map_err(err)?;
    if final_matrix.matrix().statement_digest() != runtime.bank().matrix_digest()
        || final_matrix.block_vk() != parts.block_vk()
        || final_matrix.parent_vk() != parts.parent_vk()
        || !final_matrix.matrix().satisfies(final_matrix.witness())
    {
        return Err("pinning candidate identities changed the complete relation".into());
    }
    drop(final_matrix);
    let encoded = parts.encode_compact().map_err(err)?;
    let decoded = v2::V2RuntimeParts::decode_compact(&encoded).map_err(err)?;
    if decoded.config() != config
        || decoded.block_vk() != parts.block_vk()
        || decoded.parent_vk() != parts.parent_vk()
    {
        return Err("runtime recipe round trip".into());
    }
    std::fs::write(settings.output.join("candidate.parts"), encoded).map_err(err)?;
    let manifest = json!({"status":"research; no release parameters selected", "m":config.outer_m(),
        "pages":PAGES, "block_seconds":config.block_time(), "activation":config.activation_height(),
        "legacy_v1_1_activation":config.schedule().v1_1_height(), "useful_rows":rows,
        "legacy_metadata":hex::encode(settings.pin), "legacy_fixture_directory":settings.fixtures,
        "matrix":hex::encode(runtime.bank().matrix_digest()), "bank":hex::encode(runtime.bank().digest()),
        "parent_vk":hex::encode(parts.parent_vk().transcript_digest()),
        "block_vk":hex::encode(parts.block_vk().transcript_digest()),
        "matrix_zstd_bytes":std::fs::metadata(matrix_path).map_err(err)?.len(),
        "setup_ms":elapsed(start), "memory":memory(), "rayon_threads":rayon::current_num_threads(),
        "cpu_backend":noid_core::cpu::selected_backend().to_string()});
    std::fs::write(
        settings.output.join("candidate.pins.json"),
        serde_json::to_vec_pretty(&manifest).map_err(err)?,
    )
    .map_err(err)?;
    println!("{}", manifest);
    Ok((runtime, origin))
}

fn write_matrix(path: &Path, matrix: &FieldR1cs) -> Result<()> {
    let file = std::fs::File::create(path.with_extension("zst.partial")).map_err(err)?;
    let mut encoder = zstd::stream::write::Encoder::new(file, 3).map_err(err)?;
    matrix.write_artifact(&mut encoder).map_err(err)?;
    encoder.flush().map_err(err)?;
    encoder.finish().map_err(err)?.sync_all().map_err(err)?;
    std::fs::rename(path.with_extension("zst.partial"), path).map_err(err)
}

pub fn audit_terminal(
    runtime: &v2::V2Runtime,
    origin: &v2::VerifiedV2Origin,
    bytes: &[u8],
    header: &BlockHeader,
    epoch: &BlockHeader,
) -> Result<()> {
    let rejects = |bytes: &[u8]| match v2::decode_terminal(runtime, bytes) {
        Err(_) => true,
        Ok(t) => v2::verify_terminal(runtime, origin, &t, header, epoch).is_err(),
    };
    for length in [0, 41, bytes.len() - 1] {
        if !rejects(&bytes[..length]) {
            return Err("truncated terminal accepted".into());
        }
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    if !rejects(&trailing) {
        return Err("trailing bytes accepted".into());
    }
    for (offset, value) in [(0, 4), (0, 5), (41, 1), (41, 2)] {
        let mut altered = bytes.to_vec();
        altered[offset] = value;
        if !rejects(&altered) {
            return Err("wrong terminal version/class accepted".into());
        }
    }
    let lanes = runtime.bank().config().io_spec().io_len;
    for lane in 0..lanes {
        let mut altered = bytes.to_vec();
        altered[42 + 32 + lane * 16] ^= 1;
        if !rejects(&altered) {
            return Err(format!("altered public IO lane {lane} accepted"));
        }
    }
    let mut altered = bytes.to_vec();
    altered[42 + 32 + lanes * 16] ^= 1;
    if !rejects(&altered) {
        return Err("altered zerocheck accepted".into());
    }
    let mut other = *header;
    other.tx_root[0] ^= 1;
    let t = v2::decode_terminal(runtime, bytes).map_err(err)?;
    if v2::verify_terminal(runtime, origin, &t, &other, epoch).is_ok() {
        return Err("terminal accepted a different sealed header".into());
    }
    println!(
        "{}",
        json!({"phase":"terminal_adversarial_checks", "rejected":lanes+10,
        "height":header.height})
    );
    Ok(())
}
