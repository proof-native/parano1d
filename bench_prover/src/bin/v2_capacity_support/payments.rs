//! One independent owner, one input and two outputs per logical transaction.
//! Wallet proof creation is timed separately from the miner's HistoryStep.
use super::*;

pub(super) fn measure<const PAGES: usize>(run: &mut Run, chain: &mut Chain) -> Result<()> {
    if !(1..=255).contains(&PAGES) {
        return Err("independent-owner fixture supports 1..=255 owners".into());
    }
    while chain.ordinary_slots().len() < PAGES {
        let sources = chain.ordinary_slots();
        let count = sources.len().min(PAGES - sources.len());
        if count == 0 {
            return Err("independent-owner fixture has no funding notes".into());
        }
        let mut batch = Batch::default();
        for source in sources.into_iter().take(count) {
            let value = chain.input_slot(source)?.amount;
            let amount = value.checked_sub(chain.fee(2)).ok_or("funding balance")?;
            let slots = chain.empty_slots::<2>()?;
            batch.ordinary(chain, source, outputs(slots, amount, address(1)), true)?;
        }
        run.block::<PAGES>(chain, "independent_owner_funding", batch)?;
    }

    chain.distribute_outputs();
    let mut owners = Vec::with_capacity(PAGES);
    let mut funding = Batch::default();
    for (index, source) in chain.ordinary_slots().into_iter().take(PAGES).enumerate() {
        let signer = (index + 1) as u8;
        let amount = chain
            .input_slot(source)?
            .amount
            .checked_sub(chain.fee(1))
            .ok_or("owner funding balance")?;
        let [slot] = chain.empty_slots()?;
        funding.ordinary(
            chain,
            source,
            [
                TxOutput {
                    slot_index: slot,
                    amount,
                    owner: address(signer),
                },
                TxOutput::dummy(),
            ],
            false,
        )?;
        owners.push((slot, signer));
    }
    run.block::<PAGES>(chain, "fund_independent_owners", funding)?;

    for sample in 0..run.settings.samples {
        for count in [2, PAGES] {
            let mut batch = Batch::default();
            let mut spent_segments = BTreeSet::new();
            let mut touched_segments = BTreeSet::new();
            for (source, signer) in owners.iter_mut().take(count) {
                let amount = chain
                    .input_slot(*source)?
                    .amount
                    .checked_sub(chain.fee(2))
                    .ok_or("independent payment balance")?;
                let slots = chain.empty_slots::<2>()?;
                spent_segments.insert(*source >> 16);
                touched_segments.insert(*source >> 16);
                touched_segments.extend(slots.map(|slot| slot >> 16));
                let mut payment_outputs = outputs(slots, amount, address(*signer));
                let recipient = if usize::from(*signer) == PAGES {
                    1
                } else {
                    *signer + 1
                };
                payment_outputs[1].owner = address(recipient);
                batch.ordinary_as(chain, *source, payment_outputs, true, *signer)?;
                *source = slots[0];
            }
            println!(
                "{}",
                json!({"phase":"payment_fixture", "sample":sample,
                "independent_owners":count, "logical_transactions":count,
                "inputs":count, "outputs":2*count, "spent_segments":spent_segments.len(),
                "touched_segments":touched_segments.len()})
            );
            run.block::<PAGES>(
                chain,
                &format!("payment_sample_{sample}_owners_{count}_inputs_1_outputs_2"),
                batch,
            )?;
        }
    }
    run.block::<PAGES>(chain, "independent_owner_empty_successor", Batch::default())?;
    println!(
        "{}",
        json!({"complete":true,"mode":"payments", "m":run.settings.config.outer_m(),
        "pages":PAGES,"samples":run.settings.samples,"tip":chain.parent().height,"memory":memory()})
    );
    Ok(())
}

fn outputs(slots: [u32; 2], amount: u64, owner: Address) -> [TxOutput; 2] {
    [
        TxOutput {
            slot_index: slots[0],
            amount: amount / 2,
            owner,
        },
        TxOutput {
            slot_index: slots[1],
            amount: amount - amount / 2,
            owner,
        },
    ]
}
