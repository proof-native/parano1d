// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

use crate::app::{App, Message, CONTRACTS_SCROLL_ID};
use crate::contracts::{Action, DetailTab, Field, Info, Kind, Tab, UseAction};
use crate::i18n::{text, text_input};
use crate::model::format_micronoid;
use crate::theme::{self, ButtonKind};
use iced::widget::{
    button, column, container, mouse_area, opaque, row, scrollable, stack, text_editor, Space,
};
use iced::{Alignment, Element, Length, Padding};

fn command(label: &'static str, action: Action, enabled: bool) -> Element<'static, Message> {
    control(label, action, enabled, ButtonKind::Command)
}

fn primary(label: &'static str, action: Action, enabled: bool) -> Element<'static, Message> {
    control(label, action, enabled, ButtonKind::Primary)
}

fn control(
    label: &'static str,
    action: Action,
    enabled: bool,
    kind: ButtonKind,
) -> Element<'static, Message> {
    button(text(label).size(13))
        .padding([8, 12])
        .on_press_maybe(enabled.then_some(Message::Contract(action)))
        .style(move |_, status| theme::button(kind, status))
        .into()
}

fn panel<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .padding(14)
        .width(Length::Fill)
        .style(theme::surface)
        .into()
}

fn field<'a>(
    label: &'static str,
    value: &'a str,
    key: Field,
    enabled: bool,
) -> Element<'a, Message> {
    let input = text_input(label, value)
        .padding(8)
        .size(13)
        .style(theme::text_input);
    let input = if enabled {
        input.on_input(move |v| Message::Contract(Action::Edit(key, v)))
    } else {
        input
    };
    column![text(label).size(12).color(theme::MUTED), input]
        .spacing(5)
        .width(Length::Fill)
        .into()
}

fn pair<'a>(
    left: Element<'a, Message>,
    right: Element<'a, Message>,
    compact: bool,
) -> Element<'a, Message> {
    if compact {
        column![left, right].spacing(10).into()
    } else {
        row![
            container(left).width(Length::Fill),
            container(right).width(Length::Fill)
        ]
        .spacing(10)
        .align_y(Alignment::Start)
        .into()
    }
}

fn value<'a>(label: &'static str, value: impl ToString) -> Element<'a, Message> {
    column![
        text(label).size(12).color(theme::DIM),
        text(value)
            .size(13)
            .wrapping(iced::widget::text::Wrapping::Glyph),
    ]
    .spacing(5)
    .width(Length::Fill)
    .into()
}

fn short_address(value: &str) -> String {
    if value.chars().count() <= 28 {
        return value.into();
    }
    let start: String = value.chars().take(15).collect();
    let end: String = value
        .chars()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{start}…{end}")
}

fn tab(label: &'static str, tab: Tab, active: Tab) -> Element<'static, Message> {
    control(
        label,
        Action::SetTab(tab),
        true,
        if tab == active {
            ButtonKind::CommandActive
        } else {
            ButtonKind::Command
        },
    )
}

pub fn view(app: &App, compact: bool) -> Element<'_, Message> {
    let state = &app.contracts;
    let tabs = row![
        tab("CREATE", Tab::Create, state.tab),
        tab("MY CONTRACTS", Tab::Mine, state.tab),
        tab("OPEN FILE", Tab::Open, state.tab),
    ]
    .spacing(5)
    .align_y(Alignment::Center);
    let body = match state.tab {
        Tab::Create => create(app, compact),
        Tab::Mine => workspace(app, compact),
        Tab::Open => open_file(app, compact),
    };
    let mut content = column![
        tabs,
        scrollable(container(body).padding(Padding::ZERO.right(10)))
            .id(CONTRACTS_SCROLL_ID)
            .height(Length::Fill)
            .style(theme::scrollable),
    ]
    .spacing(10);
    // Keep long-running authorization and verification feedback in sight.
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
    let page: Element<'_, Message> = container(content)
        .padding(12)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    if let Some(kind) = state.help {
        stack([page, help_popup(kind)])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        page
    }
}

fn workspace(app: &App, compact: bool) -> Element<'_, Message> {
    let state = &app.contracts;
    let protocol_active = state
        .protocol
        .as_ref()
        .is_some_and(|p| p.available(app.snapshot.network.height));
    let detail = if let Some(info) = &state.info {
        selected_contract(app, info, compact)
    } else {
        panel(
            column![
                text("CHOOSE A CONTRACT").size(17).color(theme::PROOF),
                text(
                    "Select one from your list, create a new contract or open a file you received."
                )
                .size(13)
                .color(theme::MUTED),
                row![
                    primary("CREATE CONTRACT", Action::SetTab(Tab::Create), !state.busy),
                    command("OPEN FILE", Action::SetTab(Tab::Open), !state.busy)
                ]
                .spacing(8),
            ]
            .spacing(14),
        )
    };
    let panes: Element<'_, Message> = if compact {
        column![library(app), detail].spacing(10).into()
    } else {
        row![
            container(library(app)).width(Length::Fixed(300.0)),
            container(detail).width(Length::Fill)
        ]
        .spacing(10)
        .align_y(Alignment::Start)
        .into()
    };
    let mut body = column![].spacing(10);
    if !protocol_active {
        let mut activation =
            column![
                text("Contracts become available automatically at the v2 activation block.")
                    .size(13)
                    .color(theme::MUTED)
            ]
            .spacing(5);
        if let Some(height) = state.protocol.as_ref().and_then(|p| p.activation_height) {
            activation = activation.push(value("ACTIVATION BLOCK", height));
        }
        body = body.push(panel(activation));
    }
    // Confirmation stays above the workspace, including when the form was scrolled.
    if let Some(review) = review(app, compact) {
        return body.push(review).into();
    }
    body.push(panes).into()
}

fn library(app: &App) -> Element<'_, Message> {
    let state = &app.contracts;
    let ready = !state.busy;
    let mut entries = column![].spacing(6);
    if state.library.is_empty() {
        entries = entries.push(
            text("Your created and imported contracts will appear here.")
                .size(13)
                .color(theme::MUTED),
        );
    }
    let mut families = std::collections::HashSet::new();
    for (index, entry) in state.library.iter().enumerate() {
        if !families.insert(entry.info.family_key()) {
            continue;
        }
        let selected = state
            .info
            .as_ref()
            .is_some_and(|info| info.family_key() == entry.info.family_key());
        entries = entries.push(
            button(
                column![
                    text(if entry.name.is_empty() {
                        entry.kind.map_or("CONTRACT", Kind::label)
                    } else {
                        &entry.name
                    })
                    .size(13),
                    text(entry.source.label()).size(11).color(theme::DIM),
                    text(short_address(&entry.info.address))
                        .size(12)
                        .color(theme::MUTED),
                ]
                .spacing(4),
            )
            .padding([11, 9])
            .width(Length::Fill)
            .on_press_maybe(ready.then_some(Message::Contract(Action::LoadSaved(index))))
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
    panel(
        column![
            row![
                text("MY CONTRACTS").size(13).color(theme::CYAN),
                Space::new().width(Length::Fill),
                text(families.len()).size(13).color(theme::DIM)
            ]
            .align_y(Alignment::Center),
            entries,
            command("REFRESH LIST", Action::Home, ready),
        ]
        .spacing(12),
    )
}

fn create(app: &App, compact: bool) -> Element<'_, Message> {
    let state = &app.contracts;
    let ready = !state.busy;
    let mut choices = column![text("CHOOSE A TEMPLATE").size(13).color(theme::CYAN)].spacing(6);
    for kind in Kind::ALL {
        let selected = state.kind == kind;
        choices = choices.push(
            button(
                row![
                    text(kind.label()).size(13),
                    Space::new().width(Length::Fill),
                    text(if selected { "●" } else { "" }).color(theme::CYAN)
                ]
                .align_y(Alignment::Center),
            )
            .padding([13, 11])
            .width(Length::Fill)
            .on_press_maybe(ready.then_some(Message::Contract(Action::Kind(kind))))
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
    let mut form = column![
        row![
            text(state.kind.label()).size(16).color(theme::PROOF),
            Space::new().width(Length::Fill),
            command("ⓘ", Action::ShowHelp(state.kind), true)
        ]
        .align_y(Alignment::Center),
        field("CONTRACT NAME", &state.draft_name, Field::DraftName, ready),
    ]
    .spacing(12);
    if state.kind == Kind::Custom {
        let editor = text_editor(&state.editor)
            .height(Length::Fixed(320.0))
            .size(13)
            .style(theme::text_editor);
        form = form.push(if ready {
            editor.on_action(|action| Message::Contract(Action::EditProgram(action)))
        } else {
            editor
        });
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

    form = form.push(pair(
        field(
            "INITIAL DEPOSIT (NOID)",
            &state.initial_amount,
            Field::InitialAmount,
            ready,
        ),
        field(
            "NETWORK FEE (EMPTY IS AUTOMATIC)",
            &state.initial_fee,
            Field::InitialFee,
            ready,
        ),
        compact,
    ));
    let active = ready
        && state
            .protocol
            .as_ref()
            .is_some_and(|p| p.available(app.snapshot.network.height));
    form = form.push(
        row![
            primary("CREATE & FUND", Action::CreateAndFund, active),
            command("SAVE WITHOUT DEPOSIT", Action::Create, ready),
        ]
        .spacing(8),
    );
    let mut page = column![row![
        text("NEW CONTRACT").size(15).color(theme::PROOF),
        Space::new().width(Length::Fill),
        text(format!("BLOCK #{}", app.snapshot.network.height))
            .size(12)
            .color(theme::DIM)
    ]
    .align_y(Alignment::Center),]
    .spacing(10);
    if let Some(review) = review(app, compact) {
        return page.push(review).into();
    }
    if compact {
        page = page.push(panel(choices)).push(panel(form));
    } else {
        page = page.push(
            row![
                container(panel(choices)).width(Length::FillPortion(4)),
                container(panel(form)).width(Length::FillPortion(8))
            ]
            .spacing(10)
            .align_y(Alignment::Start),
        );
    }
    page.into()
}

fn help_popup(kind: Kind) -> Element<'static, Message> {
    let explanation = match kind {
        Kind::Payment => "The payee can collect before expiry. Your active address can recover the balance from the expiry block onward.",
        Kind::Vault => "Your active address owns the vault. Neither you nor another key can withdraw before the unlock block.",
        Kind::Allowance => "The spending key can make capped payments while preserving a reserve. The cap applies to each call. Your active address recovers the balance at the recovery block.",
        Kind::Budget => "The budget includes payments and fees. Unused budget does not carry over. The first call after a period expires starts a new period.",
        Kind::Recurring => "The payee claims a fixed prepaid payment when due. Missed charges do not accumulate. Each successful call starts the next period.",
        Kind::Vesting => "Fixed tranches unlock on a block schedule. Missed tranches can be claimed one per call. The beneficiary can withdraw the remainder at maturity.",
        Kind::Custom => "Write a bounded integer program or edit a template. Creating edited terms makes a new contract; it does not change an existing balance.",
    };

    let example = match kind {
        Kind::Payment => "Example: reserve 10 NOID for a recipient. They collect before expiry; you can recover the remainder from the expiry block.",
        Kind::Vault => "Example: lock savings until a chosen block. The owner can withdraw after that block.",
        Kind::Allowance => "Example: give a spending key access to a funded balance with a maximum payment per call and a protected reserve.",
        Kind::Budget => "Example: allow up to 10 NOID of payments and fees per period. Unused budget does not carry over.",
        Kind::Recurring => "Example: prepay a recurring allowance. The recipient claims each due payment; nothing is charged automatically.",
        Kind::Vesting => "Example: unlock 5 NOID at each interval. The beneficiary submits a call to claim each tranche.",
        Kind::Custom => "Two saved u64 counters, two scratch registers, up to 16 instructions. Use decimal strings for counters and constants; amounts in the program are micronoid.",
    };
    let mut content = column![
        row![text(kind.label()).size(17).color(theme::PROOF), Space::new().width(Length::Fill), command("CLOSE", Action::CloseHelp, true)].spacing(12).align_y(Alignment::Center),
        text(explanation).size(14),
        text(example).size(13).color(theme::MUTED),
        text("Creating and funding requires your confirmation. Later actions depend on the contract rules and the active wallet address.").size(13).color(theme::MUTED),
        text("Contract schedules use block heights. Each deposit has its own balance and counters.").size(13).color(theme::MUTED),
    ].spacing(18);
    if kind == Kind::Custom {
        content = content.push(text("Operations: move, add, subtract, min, max, less_than, equal, assert_equal, assert_less_or_equal. Conditions use the deadline, closing flag, payment flag or a scratch boolean. Arithmetic overflow rejects the call.").size(13).color(theme::MUTED));
    }
    let modal = container(scrollable(content).style(theme::scrollable))
        .padding(22)
        .max_width(680)
        .max_height(480)
        .style(theme::surface_alt);
    let backdrop = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::overlay),
    )
    .on_press(Message::Contract(Action::CloseHelp));
    stack([
        backdrop.into(),
        container(opaque(modal))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into(),
    ])
    .into()
}

fn permissions(label: &'static str, payment: bool, close: bool) -> Element<'static, Message> {
    column![
        text(label).size(12).color(theme::DIM),
        row![
            text("PAYMENT").size(12),
            text(if payment { "allowed" } else { "disabled" })
                .size(12)
                .color(if payment { theme::CYAN } else { theme::DIM })
        ]
        .spacing(8),
        row![
            text("WITHDRAWAL").size(12),
            text(if close { "allowed" } else { "disabled" })
                .size(12)
                .color(if close { theme::CYAN } else { theme::DIM })
        ]
        .spacing(8),
    ]
    .spacing(5)
    .width(Length::Fill)
    .into()
}

fn review(app: &App, compact: bool) -> Option<Element<'_, Message>> {
    let state = &app.contracts;
    let (request, summary) = state.review.as_ref()?;
    let creation = matches!(
        request,
        crate::contracts::Request::Fund {
            creation: Some(_),
            ..
        }
    );
    if creation != (state.tab == Tab::Create) {
        return None;
    }
    let ready = !state.busy;
    let active = ready
        && state
            .protocol
            .as_ref()
            .is_some_and(|p| p.available(app.snapshot.network.height));
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
    if let crate::contracts::Request::Fund {
        info, amount, fee, ..
    } = request
    {
        review = review
            .push(pair(
                value("DEPOSIT AMOUNT (NOID)", format_micronoid(*amount)),
                value("NETWORK FEE (NOID)", format_micronoid(*fee)),
                compact,
            ))
            .push(pair(
                value("DEADLINE BLOCK", info.deadline_height),
                value(
                    "MAXIMUM CALL FEE (NOID)",
                    format_micronoid(info.max_fee_micronoid),
                ),
                compact,
            ))
            .push(pair(
                value("SPENDING AUTHORITY BEFORE DEADLINE", &info.claim_authority),
                value("SPENDING AUTHORITY FROM DEADLINE", &info.recovery_authority),
                compact,
            ));
    }
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
            primary("CONFIRM", Action::Confirm, active),
            command("CANCEL", Action::CancelReview, ready)
        ]
        .spacing(8),
    );

    Some(
        container(review)
            .width(Length::Fill)
            .padding(14)
            .style(theme::surface_alt)
            .into(),
    )
}

fn selected_contract<'a>(app: &'a App, info: &'a Info, compact: bool) -> Element<'a, Message> {
    let state = &app.contracts;
    let ready = !state.busy;
    let entry = state
        .library
        .iter()
        .find(|entry| entry.info.family_key() == info.family_key());
    let name = entry
        .filter(|e| !e.name.is_empty())
        .map(|e| e.name.as_str())
        .unwrap_or("CONTRACT");
    let header = panel(
        column![
            row![
                text(name).size(18).color(theme::PROOF),
                Space::new().width(Length::Fill),
                command("SHARE CONTRACT", Action::Save, ready)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![
                text(short_address(&info.address))
                    .size(12)
                    .color(theme::MUTED),
                super::copy_value_button(
                    &info.address,
                    app.copied_value.as_deref() == Some(&info.address)
                )
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            role(app, info),
        ]
        .spacing(10),
    );
    let tabs = row![
        control(
            "ACTIONS",
            Action::SetDetail(DetailTab::Actions),
            true,
            if state.detail == DetailTab::Actions {
                ButtonKind::CommandActive
            } else {
                ButtonKind::Command
            }
        ),
        control(
            "OPERATIONS & RECEIPTS",
            Action::SetDetail(DetailTab::Activity),
            true,
            if state.detail == DetailTab::Activity {
                ButtonKind::CommandActive
            } else {
                ButtonKind::Command
            }
        ),
        control(
            "RULES",
            Action::SetDetail(DetailTab::Rules),
            true,
            if state.detail == DetailTab::Rules {
                ButtonKind::CommandActive
            } else {
                ButtonKind::Command
            }
        ),
    ]
    .spacing(5);
    let body = match state.detail {
        DetailTab::Actions => contract_actions(app, info, compact),
        DetailTab::Activity => activity(app),
        DetailTab::Rules => rules(app, info, compact),
    };
    column![header, tabs, body].spacing(10).into()
}

fn role<'a>(app: &'a App, info: &'a Info) -> Element<'a, Message> {
    let next = app.snapshot.network.height.saturating_add(1);
    let active = &app.snapshot.active_address().address;
    let recovery = next >= info.deadline_height;
    let (authority, pay, close) = if recovery {
        (
            &info.recovery_authority,
            info.recovery_can_continue,
            info.recovery_can_close,
        )
    } else {
        (
            &info.claim_authority,
            info.claim_can_continue,
            info.claim_can_close,
        )
    };
    let future_access = !recovery
        && active == &info.recovery_authority
        && (info.recovery_can_continue || info.recovery_can_close)
        && !(active == authority && (pay || close));
    let label = if active == authority && (pay || close) {
        "YOUR ADDRESS CAN USE THIS CONTRACT"
    } else if future_access {
        "YOUR ADDRESS HAS ACCESS AFTER THE DEADLINE"
    } else {
        "VIEW ONLY FOR THE ACTIVE ADDRESS"
    };
    let mut body = column![text(label)
        .size(12)
        .color(if active == authority && (pay || close) {
            theme::CYAN
        } else {
            theme::MUTED
        })]
    .spacing(5);
    if future_access {
        body = body.push(
            row![
                text("ACCESS FROM BLOCK").size(12).color(theme::DIM),
                text(info.deadline_height).size(12).color(theme::CYAN)
            ]
            .spacing(8),
        );
    }
    body.into()
}

fn contract_actions<'a>(app: &'a App, info: &'a Info, compact: bool) -> Element<'a, Message> {
    let state = &app.contracts;
    let ready = !state.busy;
    let active = ready
        && state
            .protocol
            .as_ref()
            .is_some_and(|p| p.available(app.snapshot.network.height));
    let recovery = app.snapshot.network.height.saturating_add(1) >= info.deadline_height;
    let (authority, recipient, payable, closable) = if recovery {
        (
            &info.recovery_authority,
            &info.recovery_recipient,
            info.recovery_can_continue,
            info.recovery_can_close,
        )
    } else {
        (
            &info.claim_authority,
            &info.claim_recipient,
            info.claim_can_continue,
            info.claim_can_close,
        )
    };
    let own = authority == &app.snapshot.active_address().address;
    let mut balances = column![row![
        text("CONTRACT BALANCES").size(14).color(theme::CYAN),
        Space::new().width(Length::Fill),
        command("REFRESH", Action::Refresh, ready)
    ]
    .align_y(Alignment::Center),]
    .spacing(10);
    if let Some(instances) = &state.instances {
        if instances.slots.is_empty() {
            balances = balances.push(text("No available balance at this contract state. You can add funds or check another saved state below.").size(13).color(theme::MUTED));
        }
        for (index, slot) in instances.slots.iter().enumerate() {
            let selected = state.selected == Some(slot.slot_index);
            balances = balances.push(
                button(
                    row![
                        text(if selected { "●" } else { "○" }).size(13),
                        text(format!("{} NOID", format_micronoid(slot.value)))
                            .size(15)
                            .color(theme::ACCENT),
                        Space::new().width(Length::Fill),
                        text(format!("DEPOSIT {}", index + 1))
                            .size(12)
                            .color(theme::MUTED),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
                .padding([11, 12])
                .width(Length::Fill)
                .on_press_maybe(ready.then_some(Message::Contract(Action::Select(slot.slot_index))))
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
        balances = balances.push(
            text(format!("Checked at block {}", instances.height))
                .size(12)
                .color(theme::DIM),
        );
        if instances.slots.len() > 1 {
            balances = balances.push(text("Each deposit has its own balance and counters. Select the one you want to use.").size(12).color(theme::MUTED));
        }
        if instances.next_slot.is_some() {
            balances = balances.push(command("MORE DEPOSITS", Action::NextPage, ready));
        }
    } else {
        balances = balances.push(
            text("Checking balances / awaiting confirmation…")
                .size(13)
                .color(theme::MUTED),
        );
    }
    if let Some(known) = &state.known_states {
        balances = balances.push(
            text(format!("Other states checked at block {}", known.height))
                .size(12)
                .color(theme::DIM),
        );
        for (index, entry) in known
            .states
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.has_balance && entry.object.address != info.address)
        {
            balances = balances.push(command_owned(
                format!(
                    "OPEN AVAILABLE STATE · {}",
                    short_address(&entry.object.address)
                ),
                Action::UseKnownState(index),
                ready,
            ));
        }
        if known.next_root.is_some() {
            balances = balances.push(command("MORE SAVED STATES", Action::NextStates, ready));
        }
    }
    if state.candidate.is_some() {
        balances = balances.push(command(
            "CHECK UPDATED BALANCE",
            Action::UseCandidate,
            ready,
        ));
    }
    let close_label = if recovery {
        "WITHDRAW / RETURN"
    } else {
        "COLLECT BALANCE"
    };
    let mode = |label, mode| {
        control(
            label,
            Action::Use(mode),
            ready,
            if state.use_action == mode {
                ButtonKind::CommandActive
            } else {
                ButtonKind::Command
            },
        )
    };
    let choices = column![
        pair(
            mode("ADD FUNDS", UseAction::Fund),
            mode("MAKE PAYMENT", UseAction::Pay),
            compact
        ),
        pair(
            mode(close_label, UseAction::Close),
            mode("CALL WITHOUT PAYMENT", UseAction::Continue),
            compact
        ),
    ]
    .spacing(6);
    let mut form = column![
        text("CHOOSE AN ACTION").size(13).color(theme::CYAN),
        choices
    ]
    .spacing(12);
    let (label, action, enabled) = match state.use_action {
        UseAction::Fund => {
            form = form.push(field(
                "DEPOSIT AMOUNT (NOID)",
                &state.amount,
                Field::Amount,
                ready,
            ));
            (
                "REVIEW DEPOSIT",
                Action::ReviewFund,
                active && info.has_program_details(),
            )
        }
        UseAction::Pay => {
            form = form.push(field(
                "PAYMENT AMOUNT (NOID)",
                &state.amount,
                Field::Amount,
                ready,
            ));
            if info.unrestricted_payout_recipient {
                form = form.push(field(
                    "PAYMENT RECIPIENT",
                    &state.payout,
                    Field::Payout,
                    ready,
                ));
            } else {
                form = form.push(value("PAYMENT RECIPIENT", recipient));
            }
            (
                "REVIEW PAYMENT",
                Action::ReviewPay,
                active && own && payable && state.selected.is_some(),
            )
        }
        UseAction::Close => {
            form = form.push(value("CLOSING RECIPIENT", recipient));
            form = form.push(text("The selected deposit is closed. Its remaining balance, less the fee, goes to this recipient.").size(13).color(theme::MUTED));
            (
                "REVIEW WITHDRAWAL",
                Action::ReviewClose,
                active && own && closable && state.selected.is_some(),
            )
        }
        UseAction::Continue => {
            form = form.push(text("Run the program without making a payment. The network fee comes from the selected balance.").size(13).color(theme::MUTED));
            (
                "REVIEW CONTRACT CALL",
                Action::ReviewContinue,
                active && own && payable && state.selected.is_some() && !info.policy_only(),
            )
        }
    };
    form = form.push(field(
        "NETWORK FEE (EMPTY IS AUTOMATIC)",
        &state.fee,
        Field::Fee,
        ready,
    ));
    if !enabled && state.use_action != UseAction::Fund {
        form = form.push(
            text(if state.selected.is_none() {
                "Select an available deposit first."
            } else if !own {
                "The active address cannot perform this action at the next block."
            } else {
                "The contract rules do not allow this action at the next block."
            })
            .size(12)
            .color(theme::MUTED),
        );
    }
    form = form.push(primary(label, action, enabled));
    column![panel(balances), panel(form)].spacing(10).into()
}

fn command_owned(label: String, action: Action, enabled: bool) -> Element<'static, Message> {
    button(text(label).size(12))
        .padding([8, 10])
        .on_press_maybe(enabled.then_some(Message::Contract(action)))
        .style(|_, status| theme::button(ButtonKind::Command, status))
        .into()
}

fn policy_rules<'a>(
    info: &'a Info,
    compact: bool,
    expanded: bool,
    ready: bool,
) -> Element<'a, Message> {
    let before = column![
        permissions(
            "BEFORE DEADLINE",
            info.claim_can_continue,
            info.claim_can_close
        ),
        value("SPENDING AUTHORITY", &info.claim_authority),
        value("CLOSING RECIPIENT", &info.claim_recipient)
    ]
    .spacing(12);
    let after = column![
        permissions(
            "FROM DEADLINE",
            info.recovery_can_continue,
            info.recovery_can_close
        ),
        value("SPENDING AUTHORITY", &info.recovery_authority),
        value("CLOSING RECIPIENT", &info.recovery_recipient)
    ]
    .spacing(12);
    let mut content = column![
        pair(
            value("DEADLINE BLOCK", info.deadline_height),
            value(
                "MAXIMUM CALL FEE (NOID)",
                format_micronoid(info.max_fee_micronoid)
            ),
            compact
        ),
        pair(
            value(
                "PER-CALL PAYMENT LIMIT (NOID)",
                format_micronoid(info.max_payout_micronoid)
            ),
            value(
                "MINIMUM RESERVE (NOID)",
                format_micronoid(info.min_retained_micronoid)
            ),
            compact
        ),
        text("The payout cap and reserve apply to continuing calls.")
            .size(12)
            .color(theme::MUTED),
        pair(before.into(), after.into(), compact),
        text(if info.unrestricted_payout_recipient {
            "Continuing payments may go to any recipient."
        } else {
            "Continuing payments use the closing recipient of the active branch."
        })
        .size(12)
        .color(theme::MUTED),
    ]
    .spacing(15);
    if !info.policy_only() {
        content = content
            .push(
                text("CUSTOM PROGRAM — ADDITIONAL CONDITIONS APPLY")
                    .size(13)
                    .color(theme::PROOF),
            )
            .push(value(
                "CURRENT STATE",
                format!("state0 = {} · state1 = {}", info.state[0], info.state[1]),
            ))
            .push(command(
                if expanded {
                    "HIDE PROGRAM"
                } else {
                    "SHOW PROGRAM"
                },
                Action::ToggleProgram,
                ready,
            ));
        if expanded {
            for (step, instruction) in info.program.iter().enumerate() {
                content = content
                    .push(text(format!("#{:02} {}", step + 1, instruction.formula(step))).size(12));
            }
        }
    }
    content.into()
}

fn rules<'a>(app: &'a App, info: &'a Info, compact: bool) -> Element<'a, Message> {
    let state = &app.contracts;
    let ready = !state.busy;
    let mut content = column![
        row![
            field("LOCAL NAME", &state.name, Field::Name, ready),
            command("SAVE NAME", Action::Rename, ready)
        ]
        .spacing(8)
        .align_y(Alignment::End),
        value("CONTRACT ADDRESS", &info.address),
        policy_rules(info, compact, state.program_expanded, ready),
    ]
    .spacing(15);
    content = content.push(command("EDIT AS NEW PROGRAM", Action::EditLoaded, ready));
    if let Some(index) = state
        .library
        .iter()
        .position(|e| e.info.family_key() == info.family_key())
    {
        content = content.push(control(
            "REMOVE FROM LIST",
            Action::Forget(index),
            ready,
            ButtonKind::Ghost,
        ));
    }
    panel(content)
}

fn activity(app: &App) -> Element<'_, Message> {
    let state = &app.contracts;
    let ready = !state.busy;
    let mut body = column![
        row![text("OPERATIONS & RECEIPTS").size(14).color(theme::CYAN), Space::new().width(Length::Fill), command("REFRESH", Action::Refresh, ready)].align_y(Alignment::Center),
        text("Recent operations saved by this wallet. A receipt becomes available after confirmation.").size(12).color(theme::MUTED),
    ].spacing(10);
    if state.operations.is_empty() {
        body = body.push(
            text("No recorded operations for this contract yet.")
                .size(13)
                .color(theme::DIM),
        );
    }
    for (index, operation) in state.operations.iter().enumerate() {
        let selected = state.selected_operation == Some(index);
        body = body.push(
            button(
                column![
                    row![
                        text(operation.kind.label()).size(13).color(theme::CYAN),
                        Space::new().width(Length::Fill),
                        text(operation.amount_micronoid.map_or_else(
                            || "AMOUNT NOT INCLUDED IN RECEIPT".into(),
                            |amount| format!("{} NOID", format_micronoid(amount))
                        ))
                        .size(13)
                    ]
                    .align_y(Alignment::Center),
                    row![
                        text(operation.status(app.snapshot.network.height))
                            .size(12)
                            .color(if operation.canonical {
                                theme::ACCENT
                            } else {
                                theme::MUTED
                            }),
                        Space::new().width(Length::Fill),
                        text(short_address(&operation.txid))
                            .size(12)
                            .color(theme::DIM)
                    ]
                    .align_y(Alignment::Center),
                ]
                .spacing(6),
            )
            .padding(12)
            .width(Length::Fill)
            .on_press_maybe(ready.then_some(Message::Contract(Action::SelectOperation(index))))
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
    if let Some((index, operation)) = state
        .selected_operation
        .and_then(|index| state.operations.get(index).map(|op| (index, op)))
    {
        let mut detail = column![
            value("TRANSACTION ID", &operation.txid),
            value(
                "NETWORK FEE (NOID)",
                operation
                    .fee_micronoid
                    .map_or_else(|| "NOT INCLUDED IN RECEIPT".into(), format_micronoid)
            )
        ]
        .spacing(12);
        if let Some(confirmation) = &operation.confirmation {
            detail = detail.push(value("INCLUSION BLOCK", confirmation.height));
        }
        detail = detail.push(primary(
            "SAVE RECEIPT",
            Action::SaveOperationReceipt(index),
            ready && operation.canonical && operation.receipt_available,
        ));
        if operation.canonical && !operation.receipt_available {
            detail = detail.push(text("Confirmed. The receipt proof is still being prepared; it will be checked again at the next block.").size(12).color(theme::MUTED));
        }
        body = body.push(panel(detail));
    }
    panel(body)
}

fn open_file(app: &App, compact: bool) -> Element<'_, Message> {
    let state = &app.contracts;
    let ready = !state.busy;
    let mut body = column![panel(column![
        text("OPEN A CONTRACT YOU RECEIVED").size(16).color(theme::PROOF),
        text("Choose a contract file or a contract receipt. Review the rules and your access before adding it to your wallet.").size(13).color(theme::MUTED),
        row![field("FILE PATH", &state.file_path, Field::FilePath, ready), command("BROWSE…", Action::BrowseFile, ready)].spacing(8).align_y(Alignment::End),
        primary("OPEN FILE", Action::OpenFile, ready && !state.file_path.trim().is_empty()),
    ].spacing(12))].spacing(10);
    if let Some(file) = &state.opened {
        let mut preview = column![
            text(if file.proof.is_some() {
                "FILE CHECKED · RECEIPT VERIFIED"
            } else {
                "CONTRACT RULES LOADED"
            })
            .size(14)
            .color(theme::CYAN),
            value("FILE", &file.file_name),
        ]
        .spacing(12);
        if let Some(call) = &file.verified_call {
            preview = preview.push(value("CALL AUTHORIZED BY", &call.authority));
        }
        if let Some(operation) = &file.operation {
            if let Some(confirmation) = &operation.confirmation {
                preview = preview.push(value("INCLUSION BLOCK", confirmation.height));
            }
            preview = preview.push(value("TRANSACTION ID", &operation.txid));
            preview = preview.push(text("The receipt confirms a past operation. Available balances are checked separately.").size(12).color(theme::MUTED));
        }
        if let Some(info) = &file.info {
            preview = preview
                .push(value("CONTRACT ADDRESS", &info.address))
                .push(role(app, info));
            preview = preview.push(policy_rules(info, compact, state.program_expanded, ready));
            if let Some(instances) = &file.instances {
                let amount: u128 = instances.slots.iter().map(|s| u128::from(s.value)).sum();
                preview = preview.push(value(
                    "AVAILABLE IN THIS PAGE (NOID)",
                    format!("{}.{:06}", amount / 1_000_000, amount % 1_000_000),
                ));
                preview = preview.push(
                    text(format!("Checked at block {}", instances.height))
                        .size(12)
                        .color(theme::DIM),
                );
                if instances.slots.is_empty() {
                    preview = preview.push(text("This state has no available balance. The file may describe a draft, a spent deposit or an older state.").size(12).color(theme::MUTED));
                }
            }
            preview = preview.push(primary(
                "ADD TO MY CONTRACTS & OPEN",
                Action::AcceptFile,
                ready,
            ));
        } else {
            preview = preview.push(text("This receipt proves a closing call. That deposit was closed by the recorded operation.").size(13).color(theme::MUTED));
        }
        body = body.push(panel(preview));
    } else {
        body = body.push(panel(
            column![
                text("WAITING FOR A FILE").size(13).color(theme::DIM),
                text("Ask the sender to use Share contract in their wallet.")
                    .size(13)
                    .color(theme::MUTED)
            ]
            .spacing(8),
        ));
    }
    body.into()
}
