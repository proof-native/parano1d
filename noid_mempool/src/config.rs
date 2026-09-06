// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Mempool configuration.

use noid_chain::consensus::wire_limits::{MAX_MEMPOOL_BYTES, MAX_MEMPOOL_TXS};

/// Configuration for the async mempool.
#[derive(Debug, Clone)]
pub struct MempoolConfig {
    /// Maximum number of admitted transactions.
    pub capacity: usize,

    /// Maximum serialized PagedSpendIntent bytes retained in RAM.
    pub max_total_intent_bytes: usize,

    /// Number of recent admitted-tx fees used to compute the dynamic fee floor.
    /// Floor = max(MIN_FEE_BASE, median(last N fees) × 0.9).
    /// From v1.1 it is applied only while the 80%/50% pressure latch is active.
    pub fee_floor_window: usize,

    /// Number of concurrent authorization verification workers (`spawn_blocking` slots).
    /// 0 = no concurrency limit; authorization verification is still required.
    /// Recommended: number of physical cores.
    pub auth_verify_workers: usize,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            capacity: MAX_MEMPOOL_TXS,
            max_total_intent_bytes: MAX_MEMPOOL_BYTES,
            fee_floor_window: 50,
            auth_verify_workers: 4,
        }
    }
}

impl MempoolConfig {
    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    pub fn with_max_total_intent_bytes(mut self, bytes: usize) -> Self {
        self.max_total_intent_bytes = bytes;
        self
    }

    pub fn with_auth_verify_workers(mut self, n: usize) -> Self {
        self.auth_verify_workers = n;
        self
    }
}
