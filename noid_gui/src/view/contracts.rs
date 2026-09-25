// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

use crate::app::{App, Message};
use crate::contracts::{Action, Field, Kind};
use crate::i18n::{text, text_input};
use crate::model::format_micronoid;
use crate::theme::{self, ButtonKind};
use iced::widget::{button, column, container, pick_list, row, scrollable, text_editor};
use iced::{Element, Length};

fn command(label: &'static str, action: Action, enabled: bool) -> Element<'static, Message> {
    button(text(label).size(13))
        .padding([8, 12])
        .on_press_maybe(enabled.then_some(Message::Contract(action)))
        .style(|_, status| theme::button(ButtonKind::Command, status))
        .into()
}

fn field<'a>(
    label: &'static str,
    value: &'a str,
    key: Field,
    enabled: bool,
) -> Element<'a, Message> {
    let input = text_input(label, value).padding(8).size(13);
    let input = if enabled {
        input.on_input(move |v| Message::Contract(Action::Edit(key, v)))
    } else {
        input
    };
    column![text(label).size(12).color(theme::MUTED), input]
        .spacing(4)
        .into()
}

fn permissions(label: &'static str, payment: bool, close: bool) -> Element<'static, Message> {
    row![
        text(label).size(12),
        text("PAYMENT").size(12),
        text(if payment { "allowed" } else { "disabled" }).size(12),
        text("WITHDRAWAL").size(12),
        text(if close { "allowed" } else { "disabled" }).size(12),
    ]
    .spacing(8)
    .into()
}

pub fn view(app: &App, _compact: bool) -> Element<'_, Message> {
    let state = &app.contracts;
    let ready = !state.busy;
    let active = ready
        && state
            .protocol
            .as_ref()
            .is_some_and(|p| p.available(app.snapshot.network.height));
    let mut body = column![
        text("CONTRACTS").size(18).color(theme::PROOF),
        text("Create spending rules, share their terms and keep proof of every call.").size(13),
        text(format!(
            "Current block: {}. Contract deadlines use block heights.",
            app.snapshot.network.height
        ))
        .size(12)
        .color(theme::MUTED),
        text("CREATE NEW TERMS").size(14).color(theme::PROOF),
        pick_list(Kind::ALL, Some(state.kind), |kind| Message::Contract(
            Action::Kind(kind)
        )),
    ]
    .spacing(10);
    let explanation = match state.kind {
        Kind::Payment => "The payee can collect before expiry. Your active address can recover the balance from the expiry block onward.",
        Kind::Vault => "Your active address owns the vault. Neither you nor another key can withdraw before the unlock block.",
        Kind::Allowance => "The spending key can make capped payments while preserving a reserve. The cap applies to each call. Your active address recovers the balance at the recovery block.",
        Kind::Budget => "The budget includes payments and fees. Unused budget does not carry over. The first call after a period expires starts a new period.",
        Kind::Recurring => "The payee claims a fixed prepaid payment when due. Missed charges do not accumulate. Each successful call starts the next period.",
        Kind::Vesting => "Fixed tranches unlock on a block schedule. Missed tranches can be claimed one per call. The beneficiary can withdraw the remainder at maturity.",
        Kind::Custom => "Write a bounded integer program or edit a template. Creating edited terms makes a new contract; it does not change an existing balance.",
    };
    let mut form = column![text(explanation).size(13).color(theme::MUTED)].spacing(8);
    if state.kind == Kind::Custom {
        let editor = text_editor(&state.editor)
            .height(Length::Fixed(320.0))
            .size(13);
        form = form.push(if ready {
            editor.on_action(|action| Message::Contract(Action::EditProgram(action)))
        } else {
            editor
        });
        form = form.push(text("Two saved u64 counters, two scratch registers, up to 16 instructions. Use decimal strings for counters and constants; amounts in the program are micronoid.").size(12).color(theme::MUTED));
        form = form.push(text("Operations: move, add, subtract, min, max, less_than, equal, assert_equal, assert_less_or_equal. Conditions use the deadline, closing flag, payment flag or a scratch boolean. Arithmetic overflow rejects the call.").size(12).color(theme::MUTED));
    } else {
        if state.kind != Kind::Vault {
            form = form.push(field(
                if matches!(state.kind, Kind::Allowance | Kind::Budget) {
                    "SPENDING KEY ADDRESS"
                } else {
                    "PAYEE ADDRESS"
                },
                &state.authority,
                Field::Authority,
                ready,
            ));
        }
        form = form.push(
            row![
                field(
                    match state.kind {
                        Kind::Payment => "EXPIRY BLOCK",
                        Kind::Vault => "UNLOCK BLOCK",
                        Kind::Vesting => "MATURITY BLOCK",
                        _ => "RECOVERY BLOCK",
                    },
                    &state.deadline,
                    Field::Deadline,
                    ready
                ),
                field(
                    "MAXIMUM CALL FEE (NOID)",
                    &state.max_fee,
                    Field::MaxFee,
                    ready
                ),
            ]
            .spacing(8),
        );
        if matches!(state.kind, Kind::Budget | Kind::Recurring | Kind::Vesting) {
            form = form.push(
                row![
                    field(
                        "FIRST PERIOD / UNLOCK BLOCK",
                        &state.start,
                        Field::Start,
                        ready
                    ),
                    field("PERIOD IN BLOCKS", &state.period, Field::Period, ready),
                ]
                .spacing(8),
            );
        }
        if matches!(
            state.kind,
            Kind::Allowance | Kind::Budget | Kind::Recurring | Kind::Vesting
        ) {
            form = form.push(field(
                if matches!(state.kind, Kind::Recurring | Kind::Vesting) {
                    "FIXED PAYMENT / TRANCHE (NOID)"
                } else {
                    "PER-CALL PAYMENT LIMIT (NOID)"
                },
                &state.max_payment,
                Field::MaxPayment,
                ready,
            ));
        }
        if state.kind == Kind::Budget {
            form = form.push(field(
                "TOTAL PERIOD BUDGET (NOID)",
                &state.budget,
                Field::Budget,
                ready,
            ));
        }
        if matches!(state.kind, Kind::Allowance | Kind::Budget) {
            form = form.push(field(
                "MINIMUM RESERVE (NOID)",
                &state.reserve,
                Field::Reserve,
                ready,
            ));
            form = form.push(field(
                "FIXED PAYMENT RECIPIENT (EMPTY ALLOWS ANY)",
                &state.payee,
                Field::Payee,
                ready,
            ));
        }
    }
    form = form.push(
        row![
            command("CREATE TERMS", Action::Create, ready),
            command("IMPORT TERMS", Action::Import, ready),
            command("VERIFY RECEIPT FILE", Action::VerifyReceipt, ready)
        ]
        .spacing(6),
    );
    body = body.push(
        container(form)
            .padding(12)
            .width(Length::Fill)
            .style(theme::surface),
    );
    let mut saved = column![
        row![text("SAVED CONTRACTS").size(14).color(theme::PROOF), command("RELOAD", Action::Home, ready)].spacing(8),
        text("Keep an exported copy of your terms. Your wallet key alone cannot restore a custom program. Saved entries can include unfunded or pending terms.").size(12).color(theme::MUTED),
    ].spacing(6);
    for (index, entry) in state.library.iter().enumerate() {
        let title = if entry.name.is_empty() {
            entry.info.address.clone()
        } else {
            format!("{} · {}", entry.name, entry.info.address)
        };
        saved = saved.push(
            row![
                button(text(title).size(12))
                    .padding(6)
                    .on_press_maybe(ready.then_some(Message::Contract(Action::LoadSaved(index))))
                    .style(|_, status| theme::button(ButtonKind::Command, status)),
                command("REMOVE FROM LIST", Action::Forget(index), ready),
            ]
            .spacing(6),
        );
    }
    body = body.push(
        container(scrollable(saved).height(Length::Fixed(140.0)))
            .padding(10)
            .style(theme::surface),
    );
    if !active {
        body = body.push(
            text("Contracts become available automatically at the v2 activation block.")
                .size(12)
                .color(theme::MUTED),
        );
        if let Some(height) = state.protocol.as_ref().and_then(|p| p.activation_height) {
            body = body.push(
                row![
                    text("ACTIVATION BLOCK").size(12),
                    text(height.to_string()).size(12)
                ]
                .spacing(8),
            );
        }
    }
    body = body.push(
        row![
            field(
                "RESTORE SAVED CONTRACT BY ADDRESS",
                &state.restore,
                Field::Restore,
                ready
            ),
            command("RESTORE", Action::Restore, ready)
        ]
        .spacing(8),
    );

    if let Some(info) = &state.info {
        let recovery = app.snapshot.network.height.saturating_add(1) >= info.deadline_height;
        let authority = if recovery {
            &info.recovery_authority
        } else {
            &info.claim_authority
        };
        let recipient = if recovery {
            &info.recovery_recipient
        } else {
            &info.claim_recipient
        };
        let own = authority == &app.snapshot.active_address().address;
        let closable = if recovery {
            info.recovery_can_close
        } else {
            info.claim_can_close
        };
        let payable = if recovery {
            info.recovery_can_continue
        } else {
            info.claim_can_continue
        };
        let mut current = column![
            row![
                text("CONTRACT TERMS").size(14).color(theme::PROOF),
                super::copy_value_button(
                    &info.address,
                    app.copied_value.as_deref() == Some(&info.address)
                )
            ]
            .spacing(8),
            text(&info.address).size(12),
            text(format!(
                "Authority before block {}: {}",
                info.deadline_height, info.claim_authority
            ))
            .size(12),
            text(format!(
                "Authority from block {}: {}",
                info.deadline_height, info.recovery_authority
            ))
            .size(12),
            permissions(
                "BEFORE DEADLINE",
                info.claim_can_continue,
                info.claim_can_close
            ),
            permissions(
                "FROM DEADLINE",
                info.recovery_can_continue,
                info.recovery_can_close
            ),
            column![
                text("CLOSING RECIPIENT BEFORE DEADLINE").size(12),
                text(&info.claim_recipient).size(12)
            ]
            .spacing(4),
            column![
                text("CLOSING RECIPIENT FROM DEADLINE").size(12),
                text(&info.recovery_recipient).size(12)
            ]
            .spacing(4),
            text(format!(
                "Maximum fee: {} NOID · Per-call payout cap: {} NOID · Reserve: {} NOID",
                format_micronoid(info.max_fee_micronoid),
                format_micronoid(info.max_payout_micronoid),
                format_micronoid(info.min_retained_micronoid)
            ))
            .size(12),
            text("The payout cap and reserve apply to continuing calls.")
                .size(12)
                .color(theme::MUTED),
            text(if info.unrestricted_payout_recipient {
                "Continuing payments may go to any recipient."
            } else {
                "Continuing payments use the closing recipient of the active branch."
            })
            .size(12),
            text(format!(
                "Recipient for a closing call at the next block: {recipient}"
            ))
            .size(12),
            row![
                command("SAVE / SHARE TERMS", Action::Save, ready),
                command("REFRESH BALANCES", Action::Refresh, ready),
                command("EDIT AS NEW PROGRAM", Action::EditLoaded, ready)
            ]
            .spacing(6),
        ]
        .spacing(8);
        current = current.push(
            row![
                field("LOCAL NAME", &state.name, Field::Name, ready),
                command("SAVE NAME", Action::Rename, ready)
            ]
            .spacing(8),
        );
        current = current.push(text("Each deposit creates a separate balance with its own counters and limits. Deposits do not merge.").size(12).color(theme::MUTED));
        if info.policy_only() {
            current = current.push(text("Policy only: no additional program conditions.").size(12));
        } else if info.has_program_details() {
            current = current
                .push(text("CUSTOM PROGRAM — ADDITIONAL CONDITIONS APPLY").size(13).color(theme::PROOF))
                .push(row![text("CODE ID").size(12), text(&info.code_id).size(12)].spacing(8))
                .push(row![text("CURRENT STATE").size(12), text(format!("state0 = {} · state1 = {}", info.state[0], info.state[1])).size(12)].spacing(8))
                .push(text("Checked unsigned 64-bit arithmetic. State and constants are exact decimal integers.").size(12).color(theme::MUTED))
                .push(text("The program reads the inclusion height, amounts, fee and call flags. Each condition is enforced by the block proof.").size(12).color(theme::MUTED));
            for (step, instruction) in info.program.iter().enumerate() {
                current = current.push(
                    text(format!("#{:02}  {}", step + 1, instruction.formula(step))).size(12),
                );
            }
        } else {
            current = current.push(text("The node did not provide the contract program. Update the node and reload the terms before funding.").size(12).color(theme::DANGER));
        }
        if let Some(instances) = &state.instances {
            current = current.push(
                text(format!("Funded contracts at block {}", instances.height))
                    .size(12)
                    .color(theme::MUTED),
            );
            if instances.slots.is_empty() {
                current = current.push(text("No spendable balance for these terms.").size(13));
            }
            for slot in &instances.slots {
                let selected = state.selected == Some(slot.slot_index);
                current = current.push(
                    button(
                        text(format!(
                            "{} {} NOID · position {} · creation {}",
                            if selected { "●" } else { "○" },
                            format_micronoid(slot.value),
                            slot.slot_index,
                            slot.creation_id
                        ))
                        .size(13),
                    )
                    .on_press_maybe(
                        ready.then_some(Message::Contract(Action::Select(slot.slot_index))),
                    )
                    .padding(8)
                    .style(move |_, status| {
                        theme::button(
                            if selected {
                                ButtonKind::CommandActive
                            } else {
                                ButtonKind::Command
                            },
                            status,
                        )
                    }),
                );
            }
            if instances.next_slot.is_some() {
                current = current.push(command("NEXT PAGE", Action::NextPage, ready));
            }
        }
        if let Some(known) = &state.known_states {
            current = current.push(
                row![
                    text("OTHER SAVED COUNTERS AND BALANCES").size(12),
                    text(format!("#{}", known.height)).size(12)
                ]
                .spacing(8),
            );
            current = current.push(text("Use a funded state after another participant calls the contract or the chain changes. All entries use the same program and spending rules.").size(12).color(theme::MUTED));
            let mut available = false;
            for (index, entry) in known.states.iter().enumerate() {
                if !entry.has_balance || entry.object.address == info.address {
                    continue;
                }
                available = true;
                current = current.push(
                    button(
                        text(format!(
                            "state0 = {} · state1 = {} · {}",
                            entry.object.state[0], entry.object.state[1], entry.object.address
                        ))
                        .size(12),
                    )
                    .on_press_maybe(
                        ready.then_some(Message::Contract(Action::UseKnownState(index))),
                    )
                    .padding(8)
                    .style(|_, status| theme::button(ButtonKind::Command, status)),
                );
            }
            if !available {
                current = current.push(
                    text("No other funded states on this page.")
                        .size(12)
                        .color(theme::MUTED),
                );
            }
            if known.next_root.is_some() {
                current = current.push(command("NEXT SAVED STATES", Action::NextStates, ready));
            }
        }
        current = current.push(
            row![
                field(
                    "AMOUNT TO FUND OR PAY (NOID)",
                    &state.amount,
                    Field::Amount,
                    ready
                ),
                field(
                    "NETWORK FEE (EMPTY IS AUTOMATIC)",
                    &state.fee,
                    Field::Fee,
                    ready
                )
            ]
            .spacing(8),
        );
        if info.unrestricted_payout_recipient {
            current = current.push(field(
                "PAYMENT RECIPIENT",
                &state.payee,
                Field::Payee,
                ready,
            ));
        }
        current = current.push(
            row![
                command(
                    "REVIEW FUNDING",
                    Action::ReviewFund,
                    active && info.has_program_details()
                ),
                command(
                    "REVIEW PAYMENT",
                    Action::ReviewPay,
                    active && own && payable && state.selected.is_some()
                ),
                command(
                    "REVIEW WITHDRAWAL",
                    Action::ReviewClose,
                    active && own && closable && state.selected.is_some()
                )
            ]
            .spacing(6),
        );
        if !info.policy_only() {
            current = current.push(command(
                "REVIEW CALL WITHOUT PAYMENT",
                Action::ReviewContinue,
                active && own && payable && state.selected.is_some(),
            ));
        }
        if state.candidate.is_some() {
            current = current.push(text("A successor is saved as a candidate. Check its current balance to establish confirmation.").size(12));
            current = current.push(command(
                "CHECK CANDIDATE BALANCE",
                Action::UseCandidate,
                ready,
            ));
        }
        body = body.push(
            container(current)
                .padding(12)
                .width(Length::Fill)
                .style(theme::surface),
        );
        body = body.push(
            row![
                field(
                    "CONFIRMED CALL TRANSACTION ID",
                    &state.receipt_txid,
                    Field::ReceiptTxid,
                    ready
                ),
                command("SAVE VERIFIED RECEIPT", Action::ExportReceipt, ready)
            ]
            .spacing(8),
        );
    }
    if let Some((request, summary)) = &state.review {
        let program_notice = match request {
            crate::contracts::Request::Fund { info, .. } if !info.policy_only() => {
                "The additional program conditions shown in the terms apply to this funding."
            }
            _ => "",
        };
        let mut review = column![
            text("REVIEW TRANSACTION").color(theme::PROOF).size(14),
            text(summary).size(13),
            text(program_notice).size(12)
        ]
        .spacing(10);
        if let crate::contracts::Request::Call { preview, .. } = request {
            review = review.push(
                row![
                    text("INCLUSION BLOCK"),
                    text(preview.call_height.to_string())
                ]
                .spacing(8),
            );
            review = review.push(
                row![
                    text("NETWORK FEE (NOID)"),
                    text(format_micronoid(preview.fee_micronoid))
                ]
                .spacing(8),
            );
            review = review.push(
                row![
                    text("REMAINING BALANCE (NOID)"),
                    text(format_micronoid(preview.retained_micronoid))
                ]
                .spacing(8),
            );
            if let Some(payout) = &preview.payout {
                review = review.push(
                    row![
                        text("PAYMENT"),
                        text(format!(
                            "{} NOID → {}",
                            format_micronoid(payout.amount_micronoid),
                            payout.address
                        ))
                    ]
                    .spacing(8),
                );
            }
            if let Some(successor) = &preview.successor {
                review = review.push(
                    row![
                        text("NEXT SAVED COUNTERS"),
                        text(format!("{} · {}", successor.state[0], successor.state[1]))
                    ]
                    .spacing(8),
                );
            }
            review = review.push(text(&preview.txid).size(12).color(theme::MUTED));
        }
        review = review.push(
            row![
                command("CONFIRM", Action::Confirm, active),
                command("CANCEL", Action::CancelReview, ready)
            ]
            .spacing(8),
        );
        body = body.push(
            container(review)
                .width(Length::Fill)
                .padding(14)
                .style(theme::surface_alt),
        );
    }
    // Authorization and receipt verification can outlast the visible form.
    // Keep feedback outside its scroll area, next to the wallet navigation.
    let mut content = column![scrollable(body)
        .height(Length::Fill)
        .style(theme::scrollable)]
    .spacing(8);
    if state.busy {
        content = content.push(
            text("WAITING FOR THE LOCAL NODE")
                .color(theme::PROOF)
                .size(13),
        );
    } else if let Some(error) = &state.error {
        content = content.push(text(error).color(theme::DANGER).size(13));
    } else if let Some(notice) = &state.notice {
        content = content.push(text(notice).color(theme::CYAN).size(13));
    }
    container(content)
        .padding(12)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
