# Books in Cells

> **A draft.** Plain text accounting done properly is what Cells is designed against (SPEC.md, "What it is for"). These are the books written with what SPEC.md writes (imports, references, formulas in the header) and the primitives its Open section proposes (`money in`, `unique`, `final`, references to a file's own rows, `check`), which no reader implements yet, so these files do not read today. Every name and amount is invented.

## What it takes from whom

- **TigerBeetle**: amounts exact at a commodity's scale; one commodity per account, so a balance is one number; movements never edited, corrected by new rows; a pending movement posted or voided by a new row that names it, resolved at most once; limits as properties of an account.
- **beancount**: pessimism. Accounts are opened before use and refuse movements once closed, every booking is checked, the order of rows means nothing.
- **hledger**: the variants of a balance assertion, periodic rows for forecasts and budgets, a conversion account for exchanges.
- **The format's own rule, one fact one row**: a movement is one row with both its sides, balanced by construction. Rows that are one transaction share a `group`. The engine derives the postings view (one signed posting per leg) that balances, assertions, lots and valuation are defined over, which is how all three tools define them.

## The files

```text
books/
├── commodities.cells   what money is held in
├── accounts.cells      every account, once
├── vendors.cells
├── balances.cells      what a statement says a balance was
├── schedules.cells     movements that recur, for forecasts
├── prices.cells
└── home/money.cells    the movements
```

`commodities.cells`:

```cells
---
type Kind = currency|stock|fund|crypto|points
---
commodity:text!,kind:Kind,scale:int>=0,name:text
EUR,currency,2,euro
USD,currency,2,us dollar
KITE,stock,0,Kite Works shares
```

`accounts.cells`:

```cells
---
import commodities from "./commodities.cells"
type Kind = assets|liabilities|equity|income|expenses
type Booking = strict|fifo|lifo|hifo|average
---
account:text!,kind:Kind,commodity:commodities.commodity,opened:date,closed:date?,never_negative:yes/no,booking:Booking?
assets:bank:main,assets,EUR,2026-01-01,,yes,
assets:cash:usd,assets,USD,2026-01-01,,yes,
assets:broker:kite,assets,KITE,2026-03-01,,yes,fifo
expenses:fuel,expenses,EUR,2026-01-01,,no,
expenses:home:rent,expenses,EUR,2026-01-01,,no,
expenses:tax:income,expenses,EUR,2026-01-01,,no,
expenses:pension,expenses,EUR,2026-01-01,,no,
income:salary,income,EUR,2026-01-01,,no,
income:gains,income,EUR,2026-01-01,,no,
equity:conversion,equity,EUR,2026-01-01,,no,
```

The hierarchy is the name (`assets:bank:main` under `assets:bank`), as in every plain text ledger. `never_negative` is a limit checked before every write; `booking` says which lot a sale draws from when it names none.

`home/money.cells`:

```cells draft
---
import accounts from "../accounts.cells"
import commodities from "../commodities.cells"
import vendors from "../vendors.cells"
type Phase = single|pending|post|void
type Code = purchase|refund|transfer|salary|tax|pension|exchange|sale|correction|opening
---
id:id!,date:date,phase:Phase final,amount:money in commodity final,commodity:commodities.commodity final,account_from:accounts.account final,account_to:accounts.account final,amount_to:money in commodity_to optional final,commodity_to:commodities.commodity? final,pending:money.id optional unique final,group:money.id?,lot:money.id?,code:Code,vendor:vendors.vendor?,description:text,tax_rate:pct?,vat:money in commodity optional=amount*tax_rate/(100+tax_rate),cleared:yes/no
a1b2c3d4,2026-07-01,single,950.00,EUR,assets:bank:main,expenses:home:rent,,,,,,purchase,,rent july,0,0.00=,yes
e5f6g7h8,2026-07-14,pending,60.00,EUR,assets:bank:main,expenses:fuel,,,,,,purchase,fuel-stop,card hold at the pump,21,10.41=,no
j9k0m1n2,2026-07-16,post,48.40,EUR,assets:bank:main,expenses:fuel,,,e5f6g7h8,,,purchase,fuel-stop,fuel,21,8.40=,yes
p3q4r5s6,2026-07-31,single,2250.00,EUR,income:salary,assets:bank:main,,,,p3q4r5s6,,salary,,payslip july net,,=,yes
t7v8w9x0,2026-07-31,single,600.00,EUR,income:salary,expenses:tax:income,,,,p3q4r5s6,,tax,,payslip july tax,,=,yes
y1z2a3b4,2026-07-31,single,150.00,EUR,income:salary,expenses:pension,,,,p3q4r5s6,,pension,,payslip july pension,,=,yes
c5d6e7f8,2026-08-02,single,100.00,EUR,assets:bank:main,assets:cash:usd,108.30,USD,,,,exchange,,cash for the trip,,=,yes
g9h0j1k2,2026-08-03,single,1200.00,EUR,assets:bank:main,assets:broker:kite,10,KITE,,,,purchase,,10 KITE at 120.00,,=,yes
m3n4p5q6,2026-09-20,single,4,KITE,assets:broker:kite,assets:bank:main,540.00,EUR,,,g9h0j1k2,sale,,sold 4 KITE at 135.00,,=,yes
r7s8t9v0,2026-09-21,single,950.00,EUR,expenses:home:rent,assets:bank:main,,,,,,correction,,rent july booked twice,0,0.00=,yes
```

## What each row shows

- **A pending movement** (`e5f6g7h8`): a card hold reserves 60.00, counted in the account's pending balance and not its posted one. The post (`j9k0m1n2`) is a new row naming it in `pending`, for 48.40, and the remaining 11.60 is released. A void is the same shape with `phase` `void`. `pending` is `unique`, so a hold is settled at most once. A hold that should expire is a `check` over its date.
- **One transaction in several rows** (the payslip): three rows sharing `group`, the id of the first (3000.00 gross = 2250.00 + 600.00 + 150.00), written by one intent, so they exist together or not at all. Rows that are related but separate transactions (an invoice and its payment) need a `link`, a different thing from a group.
- **An exchange** (`c5d6e7f8`): one row, two amounts in two commodities, the rate implied. In the postings view it is four legs through `equity:conversion`, one per commodity, so each commodity balances on its own. A fee is a second row in the same group, so the rate and the fee stay apart.
- **Lots** (`g9h0j1k2`, `m3n4p5q6`): the purchase's implied unit cost (120.00 EUR) is the lot's cost. The sale names its lot in `lot`, or leaves it blank for the account's `booking` to choose. The gain, `540.00 - 4 × 120.00 = 60.00`, is a posting to `income:gains` the engine computes, never stored.
- **A correction** (`r7s8t9v0`): rent booked twice is reversed by a new row, never by editing or deleting the first, whose money columns are `final`. Only columns that are not final (`description`, `cleared`, `vendor`) are ever edited.

## A balance that must hold

`balances.cells`:

```cells draft
---
import accounts from "./accounts.cells"
check balance_holds:
  SELECT b.id FROM balances b
  WHERE b.balance <> (SELECT COALESCE(SUM(p.amount), 0) FROM postings p
                      WHERE p.date <= b.date AND p.phase IN ('single', 'post')
                        AND (p.account = b.account OR (b.children AND p.account LIKE b.account || ':%')))
---
id:id!,date:date,account:accounts.account,balance:money in account.commodity,children:yes/no,source:text?
w2x3y4z5,2026-07-31,assets:bank:main,5188.20,no,statement july
```

The account's posted balance at the end of `date` (a statement's closing balance written on the statement's date), exact, in the account's one commodity, its subaccounts included when `children` is yes. `postings` is the engine's view, each leg signed by its account kind's normal side. Making a failing balance hold is never automatic: a tool offers it, and writes a real `opening` or `correction` row.

## The rules, and where each lives

| Rule | Where |
|---|---|
| an account exists | a reference type |
| an account is open on the movement's date | `check` |
| a movement's commodity is its accounts' | `check` |
| a transaction's legs balance per commodity | by construction, and over the postings view |
| a pending movement is settled at most once | `unique` on `pending` |
| an account never goes negative | `never_negative`, checked before every write |
| an invoice appears once | `unique` on an `invoice` column |
| amounts at the commodity's scale | `money in` |
| money never edited | `final` |

## Recurring and prices

- `schedules.cells`, `id:id!,every:monthly|quarterly|yearly,day:1..31,from:date,until:date?,amount:money in commodity,commodity:commodities.commodity,account_from:accounts.account,account_to:accounts.account,description:text`: each due occurrence is shown as a computed row (never stored: a forecast, not a movement) for forecasts and budgets. When the real movement happens, a real row is written naming the schedule.
- `prices.cells`, `id:id!,date:date,commodity:commodities.commodity,price:num>0,in:commodities.commodity,source:text?`: one price per date, commodity and currency (a `unique` over the three). Valuation (then, at a period's end, now) is a query over it, never data.
