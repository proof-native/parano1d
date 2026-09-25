# Six templates and custom programs

Templates compile to the same [integer core](core.md). The template name is a
wallet convenience, not a consensus allowlist. The Create tab and CLI expose
all six; applications can also submit a custom program through the public API.
Every amount below applies to **one funded instance**.

| Template | Before its recovery boundary | At or after the boundary |
| --- | --- | --- |
| Payment with refund | Payee closes and collects the balance minus fee | Payer closes and recovers it |
| Timelocked vault | No permitted spend, even by its owner | Owner closes and collects |
| Allowance wallet | Spending key makes capped payments, preserving a reserve | Recovery key closes |
| Period budget | Spending key spends within a fee-inclusive budget for a height period | Recovery key closes |
| Recurring payment | Payee calls for one fixed payment when due | Payer recovers the remainder |
| Gradual unlock | Beneficiary claims one fixed tranche when due | Beneficiary closes and collects the remainder |

## Payment with refund and vault

A refundable payment (`refundable_payment`) uses `payer`, `payee`,
`expiry_height` and `max_fee_micronoid`. A claim before expiry or a refund from
expiry closes the whole instance. It does not support partial collection.

A vault (`timelocked_vault`) uses `owner`, `unlock_height` and a fee ceiling.
Both authority branches belong to the owner, but only the recovery branch may
close. The owner cannot bypass the height lock with their own key.

## Allowance wallet

`allowance_wallet` separates `spending_key` and `recovery_key`. Before
`recover_at`, continuing calls obey `max_payout_micronoid`,
`min_retained_micronoid` and `max_fee_micronoid`. A fixed `payout_recipient`
restricts the destination; `null` allows arbitrary continuing recipients.
The spending key cannot close. At the boundary the recovery key can close.

The payout ceiling is **per call**, excludes the fee and does not enforce a
budget per day. Use a period budget for that purpose.

## Period budget

`period_budget_wallet` adds `start_height`, positive `period_blocks` and
`budget_micronoid`. The initial counters are `[budget, start + period]`.
Spending is forbidden before the start. Each continuing call debits both its
payment and fee from the remaining budget; a call without payment still uses
the fee portion. The effective reserve is at least the fee ceiling.

At or after the saved period boundary, the next successful call resets the
budget and sets the next boundary to **that call's height + period**. Unused
budget does not accumulate. This is a height-based window started by a call,
not a calendar-day allowance or a continuously rolling interval. Recovery
closing bypasses the claim-side budget checks.

## Recurring payment

`recurring_payment` fixes payer, payee, `first_due_height`, `period_blocks`,
`payment_micronoid`, recovery height and fee ceiling. The counters hold
`[successful_claims, next_due_height]`. A claim must include the exact payment;
the next due height becomes **the inclusion height + period**.

Missed periods do not accumulate. Someone with the payee's authority must
submit each claim, and sufficient balance must remain for the reserve and
fees. There is no background scheduler in consensus.

## Gradual unlock

`tranche_vesting` uses a beneficiary, `first_unlock_height`, positive period,
`tranche_micronoid`, `mature_at` and fee ceiling. Its counters hold
`[released_amount, next_due_height]`. Each claim pays exactly one tranche and
advances the saved due height by **one period from the previous due height**.

Missed tranches remain claimable one call at a time. At maturity the beneficiary
can close for the remainder. This is discrete vesting; value does not stream
continuously with wall-clock time.

## Selecting parameters

A day is nominally **2,880 blocks** at the 30-second target, not a timestamp
guarantee. Use headroom around height deadlines for confirmation and reorgs.
A fee ceiling of `1 NOID` permits a fee up to that amount; it does not charge
1 NOID to create terms. Too little balance or a ceiling below the required fee
can prevent a call. Funding is a separate ordinary wallet transaction.

Custom programs can combine integer counters, assertions, recipient checks
and height conditions within 16 instructions. Review all branch modes and
closing semantics. Use the [API guide](api.md) for exact constructor fields
and the [GUI guide](gui.md) for the form and review flow.
