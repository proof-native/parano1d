//! Resource boundaries on the same authenticated chain and frozen matrix.
use super::*;

pub(super) fn measure<const PAGES: usize>(run: &mut Run, chain: &mut Chain) -> Result<()> {
    let limit = run.settings.config.max_live_inputs();
    chain.distribute_outputs();
    for inputs_per_page in [4, 8] {
        fund::<PAGES>(run, chain, limit + 1)?;
        let sources = spread_sources(chain);
        if inputs_per_page == 4 {
            reject_over_budget::<PAGES>(run, chain, &sources[..limit + 1])?;
        }
        let mut batch = Batch::default();
        for sources in sources[..limit].chunks(inputs_per_page) {
            payment(&mut batch, chain, sources, 2)?;
        }
        run.block::<PAGES>(
            chain,
            &format!("boundary_inputs_{limit}_per_page_{inputs_per_page}_outputs_2"),
            batch,
        )?;
    }
    // A successor returns to the ordinary low occupancy case after every
    // segment was exercised, with the previous obligation still accumulated.
    run.block::<PAGES>(chain, "boundary_empty_successor", Batch::default())
}

pub(super) fn spread_sources(chain: &Chain) -> Vec<u32> {
    let mut seen = BTreeSet::new();
    let mut first = Vec::new();
    let mut rest = Vec::new();
    for source in chain.ordinary_slots() {
        if seen.insert(source >> 16) {
            first.push(source);
        } else {
            rest.push(source);
        }
    }
    first.extend(rest);
    first
}

pub(super) fn payment(
    batch: &mut Batch,
    chain: &mut Chain,
    sources: &[u32],
    outputs: usize,
) -> Result<()> {
    if sources.is_empty() || sources.len() > TX_INPUTS || !(1..=2).contains(&outputs) {
        return Err("boundary payment shape".into());
    }
    let mut inputs = [TxInput::dummy(); TX_INPUTS];
    let mut value = 0u64;
    for (input, &source) in inputs.iter_mut().zip(sources) {
        *input = chain.input_slot(source)?;
        value = value
            .checked_add(input.amount)
            .ok_or("boundary input sum")?;
    }
    let fee = noid_chain::consensus::fee_breakdown(
        sources.len() as u64,
        outputs as u64,
        chain.parent().active_slot_count,
        chain.parent().log_slots,
    )
    .required_total
    .max(10_000);
    let amount = value.checked_sub(fee).ok_or("boundary balance")?;
    let slots = chain.empty_slots::<2>()?;
    let page = TxPage::new(TxBody {
        epoch_anchor: chain.epoch_id(),
        fee,
        input_owner: address(1),
        inputs,
        outputs: [
            TxOutput {
                slot_index: slots[0],
                amount: if outputs == 2 { amount / 2 } else { amount },
                owner: address(1),
            },
            if outputs == 2 {
                TxOutput {
                    slot_index: slots[1],
                    amount: amount - amount / 2,
                    owner: address(1),
                }
            } else {
                TxOutput::dummy()
            },
        ],
        validity_bitmap: ((1 << sources.len()) - 1)
            | output_bitmap_bit(0)
            | if outputs == 2 {
                output_bitmap_bit(1)
            } else {
                0
            }
            | PAGED_SPEND_START_BIT
            | PAGED_SPEND_END_BIT,
        is_coinbase: false,
    })
    .map_err(err)?;
    batch.pages.push(page);
    Ok(())
}

fn fund<const PAGES: usize>(run: &mut Run, chain: &mut Chain, target: usize) -> Result<()> {
    loop {
        let sources = chain.ordinary_slots();
        if sources.len() >= target {
            return Ok(());
        }
        let count = sources.len().min(PAGES).min(target - sources.len());
        if count == 0 {
            return Err("boundary funding exhausted".into());
        }
        let mut batch = Batch::default();
        for &source in &sources[..count] {
            payment(&mut batch, chain, &[source], 2)?;
        }
        run.block::<PAGES>(chain, "boundary_funding", batch)?;
    }
}

fn reject_over_budget<const PAGES: usize>(
    run: &Run,
    chain: &mut Chain,
    sources: &[u32],
) -> Result<()> {
    let mut batch = Batch::default();
    payment(&mut batch, chain, &sources[..5], 1)?;
    for sources in sources[5..].chunks(4) {
        payment(&mut batch, chain, sources, 1)?;
    }
    let (block, temporary_state) = chain.build(&batch.pages, run.settings.config.schedule())?;
    drop(temporary_state);
    // Native rejection must be the budget gate, before capsule checking.
    match chain.input::<PAGES>(&block, &batch, &run.ghost, run.settings.config) {
        Err(error) if error.contains("BlockInputLimit") => (),
        _ => return Err("385 inputs escaped the native candidate budget".into()),
    }
    batch.prove_authorizations(block.header.height)?;
    // Bypass only the native research budget using the original wider input
    // preparer. The immutable 384-input runtime must still reject in R1CS.
    let wider = v2::V2Config::new(
        run.settings.config.outer_m(),
        PAGES,
        run.settings.config.schedule(),
    )
    .map_err(err)?;
    let input = chain.input::<PAGES>(&block, &batch, &run.ghost, wider)?;
    let frozen = v2::assemble_frozen(
        &run.runtime,
        run.origin.origin(),
        run.previous.as_ref(),
        input,
    )
    .map_err(err)?;
    if frozen.matrix().statement_digest() != run.runtime.bank().matrix_digest()
        || frozen.matrix().satisfies(frozen.witness())
    {
        return Err("385-input negative control failed in the frozen matrix".into());
    }
    println!(
        "{}",
        json!({"phase":"input_budget_adversarial_check", "inputs":sources.len(),
        "outputs":batch.pages.len(),"pages":batch.pages.len(),
        "native_rejected":true,"same_matrix":true,"r1cs_satisfied":false})
    );
    Ok(())
}
