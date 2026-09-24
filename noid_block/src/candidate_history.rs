// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Explicit scheduled-candidate witness preparation. This module is not called
//! by mainnet admission or mining; no candidate parameters are release defaults.

use noid_chain::{consensus::ConsensusError, Block};
use noid_gkr::zk_authorization::ZkAuthorizationProof;
use noid_recursive::{
    acceptance::history_step::{prepare_candidate_authorizations, v2::V2Config},
    HistoryStepBlockInput, PreparedHistoryStepGhostAuthorization, V2ContractComponentInput,
};
use noid_tx::{experimental_object::ObjectOpening, TxPage, PAGED_SPEND_V2_CONTRACT_MASK};

use crate::{HistoryStepPreparationContext, HistoryStepWitnessError};

/// Validate the schedule, body, exact State boundary, object openings and
/// capsules before producing a witness. Only PoW is deferred to nonce sealing.
pub fn prepare_candidate_input<const PAGES: usize>(
    block: &Block,
    context: HistoryStepPreparationContext<'_>,
    proofs: Vec<ZkAuthorizationProof>,
    ghost: &PreparedHistoryStepGhostAuthorization,
    openings: &[ObjectOpening],
    config: V2Config,
) -> Result<HistoryStepBlockInput<PAGES>, HistoryStepWitnessError> {
    if PAGES != config.pages()
        || block.header.height < config.activation_height()
        || openings.len() > config.contract_slots()
    {
        return Err(HistoryStepWitnessError::V2ContractShape);
    }
    context
        .start_accumulator
        .validate_local_header_boundary(context.parent_header, context.tx_epoch_anchor_header)
        .map_err(HistoryStepWitnessError::StartBoundary)?;
    super::history_step_witness::validate_parent_state_boundary(
        context.parent_header,
        context.parent_state,
    )?;
    noid_chain::consensus::validation::validate_block_checks_with_schedule(
        block,
        context.parent_header,
        context.previous_timestamps,
        context.finalized_active_counts,
        Some(context.local_time),
        context.asert_anchor,
        false,
        config.schedule(),
    )?;
    let parent_id = noid_chain::hash_block_header(context.parent_header);
    let epoch = if context
        .start_accumulator
        .height
        .is_multiple_of(noid_chain::consensus::params::TX_EPOCH_BLOCKS)
    {
        parent_id
    } else {
        context.start_accumulator.epoch_anchor_id
    };
    noid_chain::consensus::validate_block_epoch_anchors(block, epoch, parent_id)?;
    if noid_chain::compute_tx_root(&block.transactions) != block.header.tx_root {
        return Err(HistoryStepWitnessError::TransactionRootMismatch);
    }
    let stream = noid_chain::validate_block_page_stream(&block.transactions)
        .map_err(|e| ConsensusError::InvalidPagedSpend(e.to_string()))?;
    validate_candidate_resources(&stream, config)?;
    let base = stream.user_start_index;
    let mut contracts = Vec::with_capacity(openings.len());
    for (index, transaction) in block.transactions.iter().enumerate() {
        if let Some(opening) = index.checked_sub(base).and_then(|i| openings.get(i)) {
            let checked = opening
                .check_call(
                    &TxPage {
                        body: transaction.body.clone(),
                    },
                    block.header.height,
                )
                .map_err(|e| ConsensusError::ShapeMismatch(e.to_string()))?;
            contracts.push(V2ContractComponentInput {
                body_index: index,
                live: true,
                program: opening.program,
                current: opening.state,
                contexts: checked.contexts,
                next: checked.next,
                controller: opening.claim_authority.as_fields(),
                refund_authority: opening.recovery_authority.as_fields(),
                terminal: checked.terminal,
                deadline: opening.deadline,
                claim_recipient: opening.claim_recipient.as_fields(),
                refund_recipient: opening.recovery_recipient.as_fields(),
                rules: opening.rules,
            });
        } else if transaction.body.validity_bitmap & PAGED_SPEND_V2_CONTRACT_MASK != 0 {
            return Err(HistoryStepWitnessError::V2ContractShape);
        }
    }
    if contracts.len() != openings.len() {
        return Err(HistoryStepWitnessError::V2ContractShape);
    }
    let mut components =
        super::history_step_witness::build_history_step_components(block, context.parent_state)?;
    for (authorization, contract) in components.authorization_inputs.iter_mut().zip(&contracts) {
        authorization.public = noid_gkr::OwnerAuthPublicInputs::new(
            authorization.tx_body_hash,
            if block.header.height < contract.deadline {
                contract.controller
            } else {
                contract.refund_authority
            },
        );
    }
    components.v2_contract_inputs = contracts;
    let authorizations = prepare_candidate_authorizations(
        config,
        components.effective_page_count(),
        &components.authorization_inputs,
        proofs,
        ghost,
    )?;
    let end = context
        .start_accumulator
        .advance(context.parent_header, &block.header)
        .map_err(HistoryStepWitnessError::AccumulatorAdvance)?;
    HistoryStepBlockInput::try_new_candidate(
        config,
        context.start_accumulator,
        &end,
        components,
        authorizations,
        &block.header,
        context.parent_header,
    )
    .map_err(HistoryStepWitnessError::RecursiveInput)
}

fn validate_candidate_resources(
    stream: &noid_chain::BlockPageStreamFacts,
    config: V2Config,
) -> Result<(), ConsensusError> {
    use noid_chain::consensus::paged_spend::PagedSpendStreamError;
    let error = if stream.effective_page_count() > config.pages() {
        Some(PagedSpendStreamError::BlockPageLimit {
            actual: stream.effective_page_count(),
            capacity: config.pages(),
        })
    } else if usize::from(stream.live_inputs) > config.max_live_inputs() {
        Some(PagedSpendStreamError::BlockInputLimit {
            actual: usize::from(stream.live_inputs),
            capacity: config.max_live_inputs(),
        })
    } else {
        None
    };
    match error {
        Some(error) => Err(ConsensusError::InvalidPagedSpend(error.to_string())),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noid_chain::consensus::{
        forks::{ForkSchedule, V2Activation},
        paged_spend::BlockProofClass,
    };

    #[test]
    fn candidate_budget_rejects_extra_inputs_even_with_spare_touched_slots() {
        let schedule = ForkSchedule::new(Some(5), V2Activation::new(10, 30)).unwrap();
        let config = V2Config::with_input_budget(23, 96, 384, schedule).unwrap();
        let mut stream = noid_chain::BlockPageStreamFacts {
            proof_class: BlockProofClass::B255,
            groups: Vec::new(),
            page_count: 96,
            logical_count: 96,
            live_inputs: 384,
            live_outputs: 96,
            has_development_payout: false,
            user_start_index: 1,
        };
        assert!(validate_candidate_resources(&stream, config).is_ok());
        stream.live_inputs = 385;
        assert!(usize::from(stream.live_inputs + stream.live_outputs) + 1 < 577);
        assert!(validate_candidate_resources(&stream, config).is_err());
        // The original candidate and old B255 allowance remain unchanged.
        assert!(
            validate_candidate_resources(&stream, V2Config::new(23, 96, schedule).unwrap()).is_ok()
        );
        assert_eq!(BlockProofClass::B255.input_capacity(), 1020);
        stream.live_inputs = 96;
        stream.has_development_payout = true;
        assert!(validate_candidate_resources(&stream, config).is_err());
    }
}
