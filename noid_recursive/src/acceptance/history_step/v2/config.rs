use super::*;
use noid_chain::consensus::forks::ForkSchedule;
use noid_ivc_core::public_io::WitnessSlice;

/// Explicit, unfrozen research parameters. Neither environment variables nor
/// the active mainnet schedule can silently select a candidate proof bank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V2Config {
    outer_m: usize,
    pages: usize,
    schedule: ForkSchedule,
}

impl V2Config {
    pub fn new(outer_m: usize, pages: usize, schedule: ForkSchedule) -> Result<Self, V2Error> {
        let activation = schedule.v2().ok_or(V2Error::Boundary)?;
        if !(23..=25).contains(&outer_m)
            || ![25, 63, 64, 96, 127, 128, 255].contains(&pages)
            || activation.height() <= 1
            || 86_400 % activation.block_time() != 0
        {
            return Err(V2Error::Runtime);
        }
        Ok(Self {
            outer_m,
            pages,
            schedule,
        })
    }

    pub fn outer_m(self) -> usize {
        self.outer_m
    }
    pub fn pages(self) -> usize {
        self.pages
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
        [
            self.outer_m as u64,
            self.pages as u64,
            self.schedule.v1_1_height().unwrap(),
            self.activation_height(),
            self.block_time(),
        ]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect()
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
