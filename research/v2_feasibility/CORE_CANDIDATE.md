# Contract core for the next capacity measurements

The candidate preserves ordinary payments and uses permissionless objects in
the existing state tree. An object commits to its program, current state,
claim and recovery authorities, deadline height, recipients and spending rules.
Creation is an ordinary payment to that commitment. Each call consumes one
object and either retains its successor or closes it to the committed recipient.
Authorization uses the existing witness-hiding wallet capsule with the
deadline-selected authority; the block relation must prove the opening and
effects as well.

The expanded candidate uses sixteen instructions, two persistent unsigned
64-bit registers and two scratch registers reset to zero on every call.
The persistent registers occupy the existing 128-bit state commitment.
There are no loops or jumps. Each instruction selects its operands, destination
and a boolean predicate. Addition and subtraction reject overflow or underflow.
This ABI is still unfrozen; earlier object and capacity measurements used a
different program and cannot qualify this candidate.

| Opcode | Operation |
|---|---|
| 0 | Keep destination |
| 1 | Move left operand to destination |
| 2 | Checked integer addition |
| 3 | Checked integer subtraction |
| 4–5 | Unsigned minimum / maximum |
| 6–7 | Unsigned less-than / equality, returning 0 or 1 |
| 8–9 | Require equality / unsigned less-than-or-equal |

Operands select registers, a `u64` immediate, zero/one, inclusion height,
fee, payout, retained amount, input amount/creation ID, input/output slots,
four little-endian recipient words, or the deadline/action flags. The height
comes from the verified block and the body operands from its committed spine.
No separate sender-supplied execution context is accepted.

Predicates select always, before deadline, terminal, payout-present, or either
scratch register, with optional inversion. A selected scratch predicate must
equal zero or one. Skipped arithmetic cannot block recovery through an unused
overflow check. Reserved opcodes, selectors, high descriptor bits and oversized
immediates reject, including in skipped instructions.

Separate integer constraints enforce balance, a fee ceiling, a continuing
reserve, a per-call payment ceiling and allowed continue/close paths before
and after the deadline. A per-call allowance is not a daily rate limit.
All application constructors use these operations and policies, with no
deployment registry or template allowlist:

| Constructor | Enforced behavior |
|---|---|
| Refundable payment | Payee collects before expiry; payer recovers afterward |
| Time-locked vault | Owner can close only at or after the unlock height |
| Allowance wallet | Spending key obeys per-call payout, fee and reserve limits |
| Period-budget wallet | Payments **and fees** consume a persistent window budget |
| Recurring payment | One exact payment when due; next due height starts from that call |
| Tranche vesting | Fixed installments on an anchored schedule; full remainder at maturity |

The first budget window starts at its configured height. After expiry the next
successful call resets the budget and starts a fresh period; unused allowance
does not accumulate. This is a block-based window, not a trailing 24-hour limit.
Recurring payments require somebody to submit a call and do not accumulate
missed charges. Vesting retains missed installments, each claimed in a separate
call, and is discrete rather than continuously proportional. Scheduled templates
retain a closing-fee reserve. Their deadline close skips claim-only assertions.

A successor that stores the actual inclusion height is valid only at the height
used to build its signed body. Recurring calls and budget resets use this form.
Wallet/API integration must expose that admission height and rebuild and
reauthorize an unconfirmed call when it changes. Validators must reject an old
successor root; a node cannot rewrite a signed transaction. Height guards whose
result stays unchanged across several heights do not have this restriction.

Each object has one selected authority per call. These templates do not provide
threshold authorization, atomic multi-object execution, a shared token ledger,
external data feeds or autonomous transaction submission. The retained opening
and authenticated current State determine spendability; a historical receipt
alone does not establish that the object remains unspent.

Contract-call capacity is an explicit `V2Config` limit committed by the bank
identity. The measurement driver accepts `--calls=N`, including all eligible
user positions; a mandatory system record consumes one shared position.
Compare 16, 32 and full envelopes in the complete recursive relation before
choosing the final limit. Changing a witness cannot change the reserved work.
The sixteen-instruction execution component alone occupies 12,275 wires; this
excludes object commitments, authorization, State, recursion and other block
checks, so it is neither a block-size nor a performance measurement.

The measurement relation must contain these authorization, policy and
execution constraints even for empty blocks. Adding an opcode or changing
contexts or commitment encoding later can change the matrix. Test both
honest execution and malformed openings, authority substitutions, deadline
boundaries and forbidden state effects before freezing any artifact.

The existing v1 and v1.1 proof relation must remain distinct and retain its
published matrix identities. A height schedule alone does not authenticate
the first v2 proof: it must bind to the verified last legacy block and all
its outstanding matrix claims. The origin interface must allow a later
authenticated certificate without making old matrices permanent inputs to
every future release.
