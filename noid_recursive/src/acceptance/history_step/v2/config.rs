use super::*;
use noid_chain::consensus::forks::ForkSchedule;
use noid_ivc_core::public_io::WitnessSlice;

/// Explicit, unfrozen research parameters. Neither environment variables nor
/// the active mainnet schedule can silently select a candidate proof bank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V2Config {
    outer_m: usize,
    pages: usize,
    max_live_inputs: usize,
    schedule: ForkSchedule,
}

impl V2Config {
    pub fn new(outer_m: usize, pages: usize, schedule: ForkSchedule) -> Result<Self, V2Error> {
        Self::with_input_budget(
            outer_m,
            pages,
            pages
                .saturating_mul(noid_tx::TX_INPUTS)
                .min(noid_chain::consensus::params::BLOCK_MAX_LIVE_INPUTS),
            schedule,
        )
    }

    /// Explicit research profile. The bounded-input shape is deliberately a
    /// separate bank identity; it cannot change a previously frozen recipe.
    pub fn with_input_budget(
        outer_m: usize,
        pages: usize,
        max_live_inputs: usize,
        schedule: ForkSchedule,
    ) -> Result<Self, V2Error> {
        let activation = schedule.v2().ok_or(V2Error::Boundary)?;
        if !(23..=25).contains(&outer_m)
            || crate::region_sidecar::selected_zk_block_geometry_with_inputs(pages, max_live_inputs)
                .is_none()
            || activation.height() <= 1
            || 86_400 % activation.block_time() != 0
        {
            return Err(V2Error::Runtime);
        }
        Ok(Self {
            outer_m,
            pages,
            max_live_inputs,
            schedule,
        })
    }

    pub fn outer_m(self) -> usize {
        self.outer_m
    }
    pub fn pages(self) -> usize {
        self.pages
    }
    pub fn max_live_inputs(self) -> usize {
        self.max_live_inputs
    }
    pub(crate) fn block_geometry(self) -> crate::region_sidecar::SelectedZkBlockGeometry {
        crate::region_sidecar::selected_zk_block_geometry_with_inputs(
            self.pages,
            self.max_live_inputs,
        )
        .expect("checked candidate geometry")
    }
    pub(crate) fn has_bounded_inputs(self) -> bool {
        self.max_live_inputs
            != noid_chain::consensus::params::block_class_spend_capacity(self.pages)
    }
    pub fn schedule(self) -> ForkSchedule {
        self.schedule
    }
    pub fn activation_height(self) -> u64 {
        self.schedule.v2().unwrap().height()
    }
    pub fn block_time(self) -> u64 {
        self.schedule.v2().unwrap().block_time()
    }
    pub fn shape(self) -> FieldShape {
        FieldShape {
            m: self.outer_m,
            k_log: self.outer_m,
            k_skip: noid_ivc_core::zerocheck::K_SKIP,
            const_pin: Some(0),
        }
    }
    pub fn pcs_params(self) -> PcsParams {
        PcsParams {
            m: self.outer_m + noid_ivc_core::pcs::LOG_PACKING,
            log_inv_rate: 2,
            log_batch_size: 5,
            profile: Default::default(),
        }
    }
    pub(super) fn layout(self) -> IoLayout {
        let point_len = 2 * self.outer_m + 1;
        let value = bank::POINT + 2 * point_len;
        let live = value + 2;
        let acc = live + 1;
        IoLayout {
            point_len,
            value,
            live,
            acc,
            len: acc + 10,
        }
    }
    pub fn io_spec(self) -> PublicIoSpec {
        let len = self.layout().len;
        PublicIoSpec {
            io_slice: WitnessSlice {
                log2_len: len.next_power_of_two().trailing_zeros() as usize,
                index: 1,
            },
            io_len: len,
            claims: Vec::new(),
        }
    }
    pub(super) fn identity_bytes(self) -> Vec<u8> {
        let mut bytes: Vec<u8> = [
            self.outer_m as u64,
            self.pages as u64,
            self.schedule.v1_1_height().unwrap(),
            self.activation_height(),
            self.block_time(),
        ]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect();
        // Preserve existing candidate identities. Only the explicitly reduced
        // budget extends the recipe, and its versioned codec carries this lane.
        if self.has_bounded_inputs() {
            bytes.extend_from_slice(&(self.max_live_inputs as u64).to_le_bytes());
        }
        bytes
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct IoLayout {
    pub point_len: usize,
    pub value: usize,
    pub live: usize,
    pub acc: usize,
    pub len: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use noid_chain::consensus::forks::V2Activation;

    #[test]
    fn input_budget_has_an_explicit_shape_and_bank_identity() {
        let schedule = ForkSchedule::new(Some(5), V2Activation::new(10, 30)).unwrap();
        let full = V2Config::new(23, 96, schedule).unwrap();
        let bounded = V2Config::with_input_budget(23, 96, 384, schedule).unwrap();
        assert_eq!(full.max_live_inputs(), 768);
        assert_eq!(full.identity_bytes().len(), 40);
        assert_eq!(bounded.identity_bytes().len(), 48);
        assert_ne!(full.identity_bytes(), bounded.identity_bytes());
        assert_eq!(bounded.block_geometry().touched_capacity, 577);
        assert_eq!(bounded.block_geometry().segment_capacity, 256);
        assert_eq!(bounded.block_geometry().meta_b_block_log, 9);
        assert_eq!(full.block_geometry().meta_b_block_log, 10);
        assert_eq!(
            bounded.block_geometry().auth_tiles,
            full.block_geometry().auth_tiles
        );
        assert_eq!(noid_tx::TX_INPUTS, 8);
        for (pages, inputs) in [(96, 0), (96, 385), (96, 769), (63, 384)] {
            assert!(V2Config::with_input_budget(23, pages, inputs, schedule).is_err());
        }
        assert!(V2Config::new(23, usize::MAX, schedule).is_err());
    }

    #[test]
    fn capacity_sweep_keeps_input_limits_and_distinct_bank_identities() {
        let schedule = ForkSchedule::new(Some(5), V2Activation::new(10, 30)).unwrap();
        let mut identities = std::collections::HashSet::new();
        for pages in [96, 112, 120, 127, 128, 192, 223, 255] {
            for inputs in [256, 384] {
                let config = V2Config::with_input_budget(23, pages, inputs, schedule).unwrap();
                let geometry = config.block_geometry();
                assert_eq!(geometry.tier, pages);
                assert_eq!(geometry.touched_capacity, inputs + 2 * pages + 1);
                assert_eq!(geometry.auth_tiles, (pages + 1).next_power_of_two());
                assert_eq!(config.max_live_inputs(), inputs);
                assert!(identities.insert(config.identity_bytes()));
            }
        }
        assert!(V2Config::with_input_budget(23, 129, 384, schedule).is_err());
        assert!(V2Config::with_input_budget(23, 127, 257, schedule).is_err());
    }
}
