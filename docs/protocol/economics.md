# Network economics

V2 separates two responsibilities: **block height determines issuance; live
State occupancy prices the cost of persistent storage**. The rules on this
page apply from **H210537**. The reasons for changing the original model are
explained below; old network profiles are retained in the [archive](../archive/index.md).

## Unit

```text
1 NOID = 1,000,000 μNOID
```

NOID is the ticker; the wallet uses ①. Consensus uses integer μNOID, never
floating-point arithmetic. There is no fixed maximum supply because the
permanent tail continues at 1 NOID per block.

## V2 issuance

Let `H = 210537` and `I = 1051200`. For a block at height `h ≥ H`, the tier is
`min((h − H) / I, 8)` using integer division. The exact gross subsidy table is:

| Tier / nominal years from v2 | First height | NOID per block | Gross NOID per full interval |
| ---: | ---: | ---: | ---: |
| 0 | 210,537 | 16 | 16,819,200 |
| 1 | 1,261,737 | 11.30 | 11,878,560 |
| 2 | 2,312,937 | 8 | 8,409,600 |
| 3 | 3,364,137 | 5.65 | 5,939,280 |
| 4 | 4,415,337 | 4 | 4,204,800 |
| 5 | 5,466,537 | 2.83 | 2,974,896 |
| 6 | 6,517,737 | 2 | 2,102,400 |
| 7 | 7,568,937 | 1.41 | 1,482,192 |
| 8 | 8,620,137 | 1 | 1,051,200 |

The final row repeats indefinitely. The nine amounts are exact consensus
constants: **16 → 11.30 → 8 → 5.65 → 4 → 2.83 → 2 → 1.41 → 1**.
There is no square-root calculation or discretionary adjustment at a boundary.
The block relation proves the reward selected by its authenticated height.
State expansion neither advances nor resets the schedule.

An interval is a nominal 365-day year at the **30-second target**. Calendar
dates depend on actual production; the clock starts at the v2 block, not at
genesis or a release date. The eight declining intervals total **53,810,928
NOID** in gross scheduled subsidy. The first two total **28,697,760 NOID**;
the tail schedules **1,051,200 NOID per nominal year**. These are subsidy sums,
not circulating-supply forecasts: burns, unclaimed amounts and the temporary
allocation accounting affect actual issued and live value.

## Why the monetary clock changed

The initial model linked reward reductions to State expansion: starting at
50 NOID, the reward halved when sustained occupancy required a larger slot
domain. This assumed that growth of useful network activity would provide a
meaningful clock for reducing issuance.

Early mainnet operation exposed a mismatch. Live occupancy remained low while
blocks continued issuing at the initial rate. State measures **currently live
outputs**, not elapsed time, transaction volume or adoption. Spending frees
slots; consolidation reduces their count; a contract can repeatedly update a
right without growing the live set. A busy, efficient application can therefore
leave State size almost unchanged. A reduction triggered only by expansion
has no predictable horizon, and rewarding State efficiency should not make
future issuance less legible.

V2 replaces that activity-dependent monetary clock with authenticated height.
The new schedule starts at the published fork height. Operators can
review the exact constants and applications can calculate every future tier
without forecasting occupancy. Existing balances are not rescaled.

## Why this curve

The design chooses a permanent **1 NOID tail**, **four halving equivalents**
before reaching it and **two nominal years per halving**. This gives the
anchors `16 → 8 → 4 → 2 → 1` over eight nominal years. Splitting each halving
into two annual reductions gives an approximate multiplier of `1/√2`, with
the published intermediate values fixed as integer amounts.

The tail preserves an ongoing block subsidy alongside fees. The finite
transition gives a visible horizon; smaller annual steps spread later subsidy
reductions rather than concentrating each into a 50% boundary. The intermediate steps are rounded to the fixed amounts 11.30,
5.65, 2.83 and 1.41 NOID.

Changing 50 NOID per 20 seconds to 16 per 30 seconds reduces the initial gross
issuance rate by **78.67%**, at the respective targets and before any old State
halving. That is a substantial immediate reduction, followed by the scheduled
steps. Height-based rules remove the need to tune the monetary
clock to observed State growth; changing them again would require another
consensus upgrade.

## State efficiency still matters

The reason to consolidate remains economic. Within every State level,
occupancy raises the price of **net-new live slots**. That growth component is
burned. Turning several inputs into fewer outputs frees slots and avoids a
growth charge, while ordinary input/output fees still apply. Contracts pay for
their actual live footprint under the same rules. Issuance and storage pricing
therefore serve separate purposes without penalizing efficient State reuse.

## Existing allocation schedule

The existing 90% / 5% / 5% subsidy split continues for its original three
365-day target-time years from genesis. V2 does not restart that period.
Transaction fees after the growth burn remain miner-claimable.

The remaining target-time duration is converted to 30-second blocks:
`H − 1 + floor((94608000 − (H − 1) × 20) / 30) = 3223778`.
The allocation ends at **H3223778**; subsequent subsidies go entirely to miners.
V2 uses **2,880 blocks per nominal daily payout**, beginning at **H213416**.
There is no payout on H210537; the incomplete old daily interval is not paid.
The final partial v2 interval is paid at the ending height. Percentages and
recipients are unchanged.

The payout uses the reward tier at its boundary; a tier change within an
interval can leave a difference unissued. At the initial 16 NOID tier, the
miner subsidy is 14.4 NOID, and each 5% share of a complete daily interval is
2,304 NOID. Mandatory system records are derived by consensus, not by an
operator transaction. This accounting must not be mistaken for extra issuance
on top of the gross schedule.

## Fees

The minimum transaction fee consists of:

- 5,000 μNOID per logical transaction;
- 100 μNOID per live input;
- 700 μNOID per live output;
- 2,500 μNOID times occupancy pressure for each net-new live slot.

Pressure multipliers are:

| Parent-State occupancy | Multiplier |
|---:|---:|
| Below 50% | 1× |
| 50% to below 75% | 2× |
| 75% to below 90% | 4× |
| 90% and above | 8× |

The State-growth component is burned. Base, input, output and voluntary tip
components are claimable by the miner.

A consolidation that turns several inputs into one output shrinks State and
pays no growth burn.

## Dynamic relay floor

Consensus fees and local relay policy are separate. The local dynamic floor
uses the greater of 5,000 μNOID and 90% of the median fee among the last 50
transactions admitted to that node's mempool, not a network-wide fee estimate.

A raised floor applies only under pressure.
Reaching 80% of either the transaction-count limit or retained-byte limit
enables it. Falling strictly below 50% of both limits, or emptying the mempool,
resets it to the default. Between those thresholds the current pressure state
is retained. The existing limits remain 1,024 transactions and 384 MiB.

A local relay-floor increase does not by itself evict already admitted
transactions. A higher consensus fee caused by parent-State occupancy is
different: normal block cleanup removes transactions that no longer meet that
minimum and releases their local reservations.

Wallet Auto fees use the actual transaction shape, current parent-State
occupancy and the local relay floor. A local fee rejection before admission
can trigger an Auto replan within the existing three-attempt submission
budget. Manual fees and already confirmed consolidation quotes are not
silently increased. There is no extra peer-fee polling, and Auto does not
promise confirmation under every other node's policy.
