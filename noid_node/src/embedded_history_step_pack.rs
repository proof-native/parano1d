// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Preflight-authenticated embedded `HistoryStep` release pack.
//!
//! Development builds may contain no pack. Transitional builds include the
//! pinned old runtime metadata and both canonical matrices. `retired-history`
//! omits those matrices and keeps independently pinned preprocessing keys.
//! The legacy pack retains its existing explicit release preflight. New v2
//! matrices are authenticated in `build.rs` before it emits private seals.
//! Old rows, when included, may retain their derived layout in the local cache.

use noid_miner::{
    EmbeddedHistoryStepMatrixError, EmbeddedHistoryStepMatrixLeaf, EmbeddedHistoryStepMatrixSource,
    HISTORY_STEP_PACK_LEAF_COUNT,
};

pub struct EmbeddedHistoryStepPack {
    runtime_metadata: &'static [u8],
    runtime_metadata_digest: [u8; 32],
    leaves: Option<[EmbeddedHistoryStepMatrixLeaf; HISTORY_STEP_PACK_LEAF_COUNT]>,
}

impl EmbeddedHistoryStepPack {
    pub const fn runtime_metadata(&self) -> &'static [u8] {
        self.runtime_metadata
    }

    pub const fn runtime_metadata_digest(&self) -> [u8; 32] {
        self.runtime_metadata_digest
    }

    pub const fn has_legacy_matrices(&self) -> bool {
        self.leaves.is_some()
    }

    pub fn matrix_source(
        &self,
        runtime_cache_directory: Option<std::path::PathBuf>,
    ) -> Result<Box<dyn noid_recursive::HistoryStepMatrixSource>, EmbeddedHistoryStepMatrixError>
    {
        let Some(leaves) = self.leaves else {
            return Ok(Box::new(RetiredLegacyRows));
        };
        // SAFETY: this private pack is emitted only from the canonical pack
        // accepted by the explicit release preflight.
        let source = unsafe { EmbeddedHistoryStepMatrixSource::from_release_build(leaves) }?;
        Ok(Box::new(match runtime_cache_directory {
            Some(directory) => source.with_runtime_cache(directory),
            None => source,
        }))
    }

    pub fn embedded_bytes_total(&self) -> usize {
        self.runtime_metadata.len()
            + self
                .leaves
                .iter()
                .flatten()
                .map(|leaf| leaf.compressed_canonical().len())
                .sum::<usize>()
    }
}

struct RetiredLegacyRows;
impl noid_recursive::HistoryStepMatrixSource for RetiredLegacyRows {
    fn load(
        &self,
        _: noid_recursive::CanonicalHistoryStepClassId,
    ) -> Result<noid_recursive::HistoryStepMatrixLease, noid_recursive::HistoryStepMatrixSourceError>
    {
        Err(noid_recursive::HistoryStepMatrixSourceError)
    }
}

struct EmbeddedRetirementKeys {
    encoded: [&'static [u8]; 2],
    release_pins: [[u8; 32]; 2],
}

pub fn embedded_retirement_keys(
    bank: &noid_recursive::acceptance::history_step_bank::PinnedHistoryStepClassBank,
) -> Result<
    Option<noid_recursive::acceptance::history_step::v2::banked::PinnedRetirementKeys>,
    String,
> {
    GENERATED_RETIREMENT_KEYS
        .as_ref()
        .map(|keys| {
            noid_recursive::acceptance::history_step::v2::banked::PinnedRetirementKeys::from_release(
                bank,
                keys.encoded,
                keys.release_pins,
            )
            .map_err(|e| e.to_string())
        })
        .transpose()
}

include!(concat!(env!("OUT_DIR"), "/retirement_keys.rs"));

/// `None` is possible only in a pack-free development build.  `build.rs`
/// rejects a release build without its required pinned artifacts.
pub fn embedded_history_step_pack() -> Option<&'static EmbeddedHistoryStepPack> {
    GENERATED_HISTORY_STEP_PACK.as_ref()
}

include!(concat!(env!("OUT_DIR"), "/history_step_pack.rs"));

struct EmbeddedV2Pack {
    metadata: &'static [u8],
    compressed_matrices: [&'static [u8]; 2],
    release_bank: [u8; 32],
    build_seals: [noid_ivc_core::field_r1cs::BuildAuthenticatedFieldR1csSeal; 2],
}

pub fn embedded_v2_runtime() -> Result<
    Option<std::sync::Arc<noid_recursive::acceptance::history_step::v2::banked::Runtime>>,
    String,
> {
    let Some(pack) = GENERATED_V2_PACK.as_ref() else {
        return Ok(None);
    };
    let metadata = noid_miner::v2_artifacts::decode_v2_runtime_metadata_pinned(
        pack.metadata,
        pack.release_bank,
    )?;
    // SAFETY: embed_v2_pack runs the full semantic authentication before
    // staging these exact immutable bytes and emitting this private seal.
    unsafe { metadata.into_embedded_runtime(pack.compressed_matrices, pack.build_seals) }
        .map(|runtime| Some(std::sync::Arc::new(runtime)))
}

include!(concat!(env!("OUT_DIR"), "/v2_pack.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use noid_recursive::acceptance::history_step_bank::CanonicalHistoryStepClassId;

    #[test]
    fn generated_v2_pack_authenticates_both_classes_and_matches_the_profile() {
        let Some(runtime) = embedded_v2_runtime().unwrap() else {
            return; // Pack-free development build.
        };
        assert_eq!(
            runtime.bank().config().schedule(),
            noid_chain::consensus::forks::ACTIVE_SCHEDULE
        );
        for class in noid_recursive::acceptance::history_step::v2::banked::Class::ALL {
            runtime
                .prepare_matrix_cache(class)
                .expect("embedded class matrix");
        }
    }

    #[test]
    fn retired_build_cannot_load_either_legacy_class() {
        if !cfg!(feature = "retired-history") {
            return;
        }
        let pack = embedded_history_step_pack().expect("retired build requires legacy metadata");
        assert!(!pack.has_legacy_matrices());
        let source = pack.matrix_source(None).unwrap();
        for index in 0..HISTORY_STEP_PACK_LEAF_COUNT {
            assert!(source
                .load(CanonicalHistoryStepClassId::from_index(index).unwrap())
                .is_err());
        }
        let metadata = noid_miner::decode_history_step_runtime_metadata_pinned(
            pack.runtime_metadata(),
            pack.runtime_metadata_digest(),
        )
        .unwrap();
        assert!(embedded_retirement_keys(metadata.bank()).unwrap().is_some());
    }

    #[test]
    fn generated_pack_preserves_dense_class_order() {
        let Some(pack) = embedded_history_step_pack() else {
            return;
        };
        assert!(!pack.runtime_metadata().is_empty());
        for (index, leaf) in pack.leaves.iter().flatten().enumerate() {
            assert_eq!(
                leaf.class(),
                CanonicalHistoryStepClassId::from_index(index).unwrap()
            );
            assert!(!leaf.compressed_canonical().is_empty());
            assert!(leaf.build_seal().canonical_bytes() > 0);
            assert_ne!(leaf.build_seal().statement_digest(), [0; 32]);
        }
        if cfg!(feature = "retired-history") {
            assert!(pack.leaves.is_none());
            assert!(GENERATED_RETIREMENT_KEYS.is_some());
        }
    }
}
