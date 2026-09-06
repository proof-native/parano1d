// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Canonical chain-level metadata prefix for HistoryStep terminals.
//!
//! The recursive layer owns and verifies the proof envelope after this prefix.
//! Chain, storage and transport share this exact canonical codec.

use core::fmt;

pub const HISTORY_STEP_TERMINAL_VERSION: u8 = 4;
/// Shared-path terminal encoding. The mathematical proof and its matrices are
/// unchanged; only the canonical byte representation changes at V1.1 activation.
pub const HISTORY_STEP_TERMINAL_SHARED_PATH_VERSION: u8 = 5;

pub const fn history_step_terminal_wire_version(height: u64) -> u8 {
    history_step_terminal_wire_version_with_activation(
        height,
        crate::consensus::params::V1_1_ACTIVATION_HEIGHT,
    )
}

/// Pure format selection for an explicit schedule, including boundary tests.
/// Production admission uses the fixed schedule through the function above.
pub const fn history_step_terminal_wire_version_with_activation(
    height: u64,
    activation: Option<u64>,
) -> u8 {
    if crate::consensus::params::v1_1_active_with(height, activation) {
        HISTORY_STEP_TERMINAL_SHARED_PATH_VERSION
    } else {
        HISTORY_STEP_TERMINAL_VERSION
    }
}
pub const HISTORY_STEP_TERMINAL_BINDING_BYTES: usize = 1 + 8 + 32 + 1;
pub const HISTORY_STEP_TIER_SLOT_COUNT: u8 = 4;
/// One class per current tier: every class shares one frozen outer shape,
/// so the parent tier never reaches the class id.
pub const HISTORY_STEP_CLASS_COUNT: u8 = HISTORY_STEP_TIER_SLOT_COUNT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryStepTerminalMetadata {
    terminal_height: u64,
    terminal_hash: [u8; 32],
    class_id: u8,
}

impl HistoryStepTerminalMetadata {
    pub fn new(
        terminal_height: u64,
        terminal_hash: [u8; 32],
        class_id: u8,
    ) -> Result<Self, HistoryStepTerminalMetadataError> {
        if class_id >= HISTORY_STEP_CLASS_COUNT {
            return Err(HistoryStepTerminalMetadataError::InvalidClassId { actual: class_id });
        }
        Ok(Self {
            terminal_height,
            terminal_hash,
            class_id,
        })
    }

    pub fn decode_prefix(bytes: &[u8]) -> Result<Self, HistoryStepTerminalMetadataError> {
        Self::decode_prefix_with_activation(bytes, crate::consensus::params::V1_1_ACTIVATION_HEIGHT)
    }

    fn decode_prefix_with_activation(
        bytes: &[u8],
        activation: Option<u64>,
    ) -> Result<Self, HistoryStepTerminalMetadataError> {
        if bytes.len() < HISTORY_STEP_TERMINAL_BINDING_BYTES {
            return Err(HistoryStepTerminalMetadataError::Truncated);
        }
        let version = bytes[0];
        let terminal_height = u64::from_le_bytes(bytes[1..9].try_into().unwrap());
        if version
            != history_step_terminal_wire_version_with_activation(terminal_height, activation)
        {
            return Err(HistoryStepTerminalMetadataError::UnsupportedVersion { actual: version });
        }
        let terminal_hash = bytes[9..41].try_into().unwrap();
        Self::new(terminal_height, terminal_hash, bytes[41])
    }

    pub fn encode_prefix(self) -> [u8; HISTORY_STEP_TERMINAL_BINDING_BYTES] {
        let mut encoded = [0u8; HISTORY_STEP_TERMINAL_BINDING_BYTES];
        encoded[0] = history_step_terminal_wire_version(self.terminal_height);
        encoded[1..9].copy_from_slice(&self.terminal_height.to_le_bytes());
        encoded[9..41].copy_from_slice(&self.terminal_hash);
        encoded[41] = self.class_id;
        encoded
    }

    pub const fn terminal_height(self) -> u64 {
        self.terminal_height
    }

    pub const fn terminal_hash(self) -> [u8; 32] {
        self.terminal_hash
    }

    pub const fn class_id(self) -> u8 {
        self.class_id
    }

    pub const fn current_class_slot(self) -> usize {
        self.class_id as usize
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryStepTerminalMetadataError {
    Truncated,
    UnsupportedVersion { actual: u8 },
    InvalidClassId { actual: u8 },
}

impl fmt::Display for HistoryStepTerminalMetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("HistoryStep terminal metadata is truncated"),
            Self::UnsupportedVersion { actual } => {
                write!(formatter, "unsupported HistoryStep version {actual}")
            }
            Self::InvalidClassId { actual } => {
                write!(formatter, "HistoryStep class id {actual} is not canonical")
            }
        }
    }
}

impl std::error::Error for HistoryStepTerminalMetadataError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_encoding_activates_at_its_own_height_not_the_tip_height() {
        for height in [0, 1, 41, 42, 43, u64::MAX] {
            let mut bytes = HistoryStepTerminalMetadata::new(height, [7; 32], 0)
                .unwrap()
                .encode_prefix();
            for activation in [None, Some(0), Some(42), Some(u64::MAX)] {
                for version in 0..=u8::MAX {
                    bytes[0] = version;
                    assert_eq!(
                        HistoryStepTerminalMetadata::decode_prefix_with_activation(
                            &bytes, activation
                        )
                        .is_ok(),
                        version
                            == history_step_terminal_wire_version_with_activation(
                                height, activation
                            ),
                    );
                }
            }
        }
        assert_eq!(
            history_step_terminal_wire_version_with_activation(41, Some(42)),
            HISTORY_STEP_TERMINAL_VERSION
        );
        assert_eq!(
            history_step_terminal_wire_version_with_activation(42, Some(42)),
            HISTORY_STEP_TERMINAL_SHARED_PATH_VERSION
        );
    }

    #[test]
    fn truncated_prefixes_are_rejected_for_both_schedules() {
        let bytes = HistoryStepTerminalMetadata::new(42, [7; 32], 0)
            .unwrap()
            .encode_prefix();
        for activation in [None, Some(42)] {
            for length in 0..HISTORY_STEP_TERMINAL_BINDING_BYTES {
                assert_eq!(
                    HistoryStepTerminalMetadata::decode_prefix_with_activation(
                        &bytes[..length],
                        activation
                    ),
                    Err(HistoryStepTerminalMetadataError::Truncated),
                );
            }
        }
    }

    #[test]
    fn all_canonical_classes_roundtrip() {
        for class_id in 0..HISTORY_STEP_CLASS_COUNT {
            let metadata = HistoryStepTerminalMetadata::new(9, [class_id; 32], class_id)
                .expect("canonical class");
            assert_eq!(
                HistoryStepTerminalMetadata::decode_prefix(&metadata.encode_prefix()),
                Ok(metadata)
            );
        }
    }

    #[test]
    fn noncanonical_class_is_rejected() {
        let mut encoded = HistoryStepTerminalMetadata::new(9, [0xA5; 32], 3)
            .unwrap()
            .encode_prefix();
        encoded[41] = HISTORY_STEP_CLASS_COUNT;
        assert_eq!(
            HistoryStepTerminalMetadata::decode_prefix(&encoded),
            Err(HistoryStepTerminalMetadataError::InvalidClassId {
                actual: HISTORY_STEP_CLASS_COUNT,
            })
        );
    }
}
