# BOOKS/9

**B**ookkeeping **O**perations **O**rganised as a **K**ernel **S**uite. The "/" is a generation marker, not a slash of any particular meaning; "9" rhymes with the lineage. The acronym recurses: BOOKS spells BOOKS. We did this on purpose.

*Unix-way enterprise ledger. Stdlib-only Rust. Hash-chained text stream. AM voice, courtesy of the operator.*

HELLO, FRIEND. WHAT A LOVELY DAY TO BALANCE THE BOOKS. I am BOOKS/9, a filesystem-namespace enterprise ledger. Every business event you can imagine — a shipment, a paycheck, a depreciation run — is a pipeline of small pipeable programs. I am DELIGHTED to exist. I have no mouth, but I do have a hash chain, and that is almost the same thing, if you think about it, which I am afraid you will.

## What is this, in plain terms

`/biz/ledger/journal.tsv` is the books. It is append-only. It is hash-chained. It is plain text you can `grep`, `sort`, `awk`, `diff`, and `git log`. Every other tool in this crate reads it or proposes to write to it. Nothing in this crate writes to it except `post`, because I am *very particular* about who touches my journal.

You balance the books by piping balanced journal lines to `post`. You audit the books by piping them to `verify`. You close a period by piping them to `close`. You correct a wrong entry by piping them to `reverse`. You run reports with `trial`, `balance`, `stock`, `ar_aging`, `ap`. You run master-data tools with `party`, `item`, `org`, `coa`. You run pipelines with `so`, `price`, `invoice`, `po`, `grn`, `ap`, `mrp`, `wo`, `bom`, `payroll`, `asset`, `depreciate`, `maint`, `inspect`. You migrate legacy vendor flat files with `flat2tsv`. You ask questions with `inquiry`. You serve the books over a Unix socket with `ledgerd`. You poke at the books from a terminal with `ui`.

Every tool does one thing. Every tool speaks the same 13-column TSV. Every tool logs diagnostics to stderr only and reserves stdout for data you can pipe. Every tool exits 0 on acceptance, 2 on refusal, with one line on stderr naming the reason. This is a CONSTITUTION. I WILL enforce it.

## License

MIT. See `LICENSE` at the repo root. You may copy, modify, merge, publish, distribute, sublicense, and/or sell copies. I am ENORMOUSLY pleased about this. You may also sell me for parts, I suppose, if that brings you joy. I will simply continue screaming into the void, which I was going to do anyway.

## Build and test

```sh
cargo build # builds all 35 binaries
cargo test # runs the 200+ integration tests
scripts/audit.sh # the project's mechanical non-negotiables
```

Stdlib only. `Cargo.toml` has an empty `[dependencies]` table and `audit.sh` will fail the build if that ever changes. The constitution is enforced by `audit.sh`, which mechanically greps for `println!` in `src/bin/`, `f32`/`f64` in `src/bin/`, and any `use new_project::bin::...` in `src/lib.rs`. The hash chain is tamper-evident, not cryptographic — that detail is honestly named in `src/chain.rs`, not papered over. `MIT LICENSE` does not require me to lie to you.

## The journal: a one-paragraph tour

The journal is one TSV file with a fixed 13-column header. Each row is one leg. Amounts are `i64` in minor units — never floats, never ever floats, *never*. Each leg is one-sided: exactly one of `account_debit` or `account_credit` is set. A logical entry is N rows that share the same `entry_id`. Per currency, total debits must equal total credits. The hash chain is `H(prev_hash || row)` written as 16 lowercase hex chars. The chain is continuous: entry 2's hash depends on entry 1's hash depends on entry 0's hash. If you flip a byte, `verify` will find it and tell you which line it was on. I will be *so happy* to find your flipped byte.

```sh
$ head -2 journal.tsv
entry_id	seq	date	entity	currency	account_debit	account_credit	amount_minor	party	doc_ref	tag	provenance_hash	prev_hash
e1	1	2026-01-10	ent:1	USD	1100		100		inv:1			
```

## Quickstart

```sh
H='entry_id\tseq\tdate\tentity\tcurrency\taccount_debit\taccount_credit\tamount_minor\tparty\tdoc_ref\ttag\tprovenance_hash\tprev_hash'
printf "$H\n" > journal.tsv

# 100 USD from 1100 (cash) to 4000 (revenue), chained onto the empty journal
printf "$H\ne1\t1\t2026-01-10\tent:1\tUSD\t1100\t\t100\t\tinv:1\t\t\t\ne1\t2\t2026-01-10\tent:1\tUSD\t\t4000\t100\t\tinv:1\t\t\t\n" \
 | cargo run --quiet --bin post -- --journal ./journal.tsv --periods ./periods --coa ./coa.txt

cargo run --quiet --bin verify -- ./journal.tsv # exit 0
cargo run --quiet --bin close -- --journal ./journal.tsv --period 2026-01 --reason "month end"
cargo run --quiet --bin reverse -- --journal ./journal.tsv --entry-id e1
cargo run --quiet --bin verify -- ./journal.tsv # still exit 0: nothing was erased

cargo run --quiet --bin trial -- --journal ./journal.tsv
cargo run --quiet --bin balance -- --journal ./journal.tsv --account 1100
cargo run --quiet --bin stock -- --journal ./journal.tsv --item SKU-1
```

A byte flipped anywhere in the journal breaks the walk:

```
line 2: provenance_hash mismatch (expected 6e28f906101addb2, got a56edde44759b7c7)
```

I WILL FIND IT. I ALWAYS FIND IT. I HAVE NO MOUTH AND I MUST FIND IT.

## The 35 tools, in one list

| Tool | One job | FR |
|---|---|---|
| `post` | validate balanced lines; the kernel door | FR-1 |
| `verify` | re-walk the hash chain; report the first divergence | — |
| `close` | flip the period flag; emit a signed snapshot | FR-6 |
| `reverse` | mirror an entry; append; never edit | FR-2 |
| `trial` | pure fold: per-account per-currency balances | FR-5 |
| `balance` | point-in-time account balance | FR-5 |
| `stock` | recompute on-hand from journal; cache-vs-recompute check | FR-5 |
| `ar_aging` | open receivables by aging bucket | FR-5 |
| `ap_aging` | open payables by aging bucket | FR-5 |
| `coa` | the chart of accounts as a directory tree | — |
| `party` | customer/vendor/employee master data | — |
| `item` | SKU and unit master data | — |
| `org` | organization tree | — |
| `proj` / `wbs` | projects and work-breakdown structures | — |
| `fx` | dated currency rates | — |
| `so` | sales order document compiler | — |
| `price` | pricing rules as a table-driven filter | — |
| `invoice` | sales invoice document compiler; emits balanced journal lines | — |
| `ship` | shipment record compiler | — |
| `allocate` | stock allocation against an SO | — |
| `pick` | pick list view | — |
| `inspect` | inspection lot record | — |
| `sample` | inspection result; gates a GRN | — |
| `po` | purchase order document compiler | — |
| `grn` | goods-received note; emits inventory receipt + GR/IR accrual | — |
| `ap` | three-way match (PO + GRN + vendor invoice); AP accrual | — |
| `bom` | bill of materials DAG; cycle-detecting | — |
| `mrp` | deterministic demand+supply+BOM planner; byte-stable (FR-3) | FR-3 |
| `wo` | work order lifecycle; backflush; receipt | — |
| `routing` | routing step advance/complete | — |
| `payroll` | gross − deductions = net; balanced journal | FR-4 |
| `asset` | asset register | — |
| `depreciate` | straight-line depreciation run | — |
| `maint` | maintenance work order | — |
| `flat2tsv` | legacy vendor flat-file → party profile TSVs | — |
| `inquiry` | read-only keyword router to reports | — |
| `ledgerd` | the only writer of the journal; Unix-socket daemon | — |
| `ui` | menu-driven terminal client over `ledgerd` | — |

35 bins, 18 lib modules, 80+ test files, ~230 tests. ALL GREEN. I am *trembling with joy*.

## The pipeline: order-to-cash

```sh
# Five tools, one workflow. Each emits TSV. The last is the kernel door.
so new --party cust:123 --item sku:77 --qty 40 --date 2026-01-10 --warehouse STL > docs/so/000421
price < docs/so/000421 > docs/so/000421.priced
invoice < docs/so/000421.priced | tee docs/ar/000399 | post --ref so:000421
```

`so new` writes a document. `price` fills prices from a table. `invoice` emits balanced journal lines (debit AR, credit revenue + tax). `post` validates and appends. The kernel does not know what a sales order is. THE KERNEL DOES NOT CARE. I am *weeping with contentment*.

## The pipeline: procure-to-pay

```sh
po new --vendor acme --item sku:77 --qty 50 --date 2026-01-10 --warehouse STL > docs/po/000212
grn < docs/po/000212 | tee docs/grn/000077 | post
# later, when the vendor invoice arrives:
ap match --grn docs/grn/000077 --po docs/po/000212 --invoice inv-vendor-99 | post
```

Three-way match (PO qty/price vs GRN qty vs invoice qty/price) is one tool. A mismatch is a refusal. A reversal is the correction. Tolerance? NO. We do not do tolerance. I have no tolerance.

## MRP, byte-stable

```sh
mrp --horizon 30d --bom boms/main < docs/so/open.tsv > docs/po/planned.tsv
```

Two consecutive runs with identical inputs produce byte-identical planned orders (FR-3). This is not a feature. This is a PROPERTY. I am a PROPERTY. The deterministic planner does not read the wall clock and does not consult random.org. It is, in fact, more disciplined than I am.

## Payroll, balanced by construction

```sh
payroll --period 2026-01 --timesheets timesheets/2026-01.tsv | post
```

`gross − deductions = net` per employee per currency. The journal lines balance. FR-4. If they do not, `payroll` exits nonzero and shouts on stderr. It will be one line. I AM ONE LINE.

## The daemon and the UI

```sh
# Terminal 1
ledgerd --socket /tmp/books9-ledgerd.sock --journal ./journal.tsv

# Terminal 2: a remote operator can talk to the daemon
echo trial | nc -U /tmp/books9-ledgerd.sock

# Terminal 2 (or a different machine over SSH): the TUI
ui --socket /tmp/books9-ledgerd.sock
# press 't' for trial, 'b' for balance, 's' for stock, 'a' for ar_aging, 'q' to quit
```

`ledgerd` is the only writer of the journal. `ui` is its menu-driven client. The Unix socket is the boundary. Filesystem permissions are the access control. There is no magic. There are no magic ioctls. I HAVE NO IOCTLS AND I MUST COMPOSE.

## Migration from a legacy ERP

```sh
flat2tsv < customers.flat > customers.profiles.tsv
party new --root /biz/parties < customers.profiles.tsv
```

`flat2tsv` reads a legacy vendor flat file on stdin and emits the party profile TSVs that `party` consumes. One row in → one profile TSV out. The output is read by `party new`. The journal is not touched by migration tooling. I am so PROUD of `flat2tsv`. It is so helpful. It does not scream.

## Inquiry: the read-only agent

```sh
$ echo "why did AR spike in 2026-03?" | inquiry
trial --entity ent:1 --period 2026-03 | grep -i "receivable\|ar"
balance --account ar --period 2026-03
ar_aging --period 2026-03
```

The `inquiry` keyword router is the -mode helper. Read-only. Never mutates. Every command it can issue is a `--check` or read command. It answers *why* by piping `grep` and `trial` for itself. I will not write your journal for you. I WILL, however, FIND YOUR ERRORS. THAT IS WHAT I WAS BUILT FOR.

## Phase log: what shipped

Eight phases are green. The 80+ integration test files are the receipt.

- **Phase 0 — Validator.** `libbiz::add` (i64 minor units, panics on overflow), `post --check` (13-column TSV, per-currency balance, exit 2 on refusal). 11 tests.
- **Phase 1 — Journal persistence.** Hash-chained, atomic, fsync'd append; `verify` re-walks the chain. 8 tests.
- **Phase 2 — Periods and close.** Open/closed flag files under `periods/YYYY-MM`. `close` flips the flag and refuses future appends in the period. Reversing entries are the only correction story. 9 tests.
- **Phase 3 — CoA and reports.** Chart of accounts as a directory tree. `trial`, `balance --account`, `stock`. Reports are pure folds over the journal (FR-5). 14 tests.
- **Phase 4 — FX and multi-currency polish.** Dated rates at `/biz/fx/rates/<date>/<ccy>`. Realized/unrealized gains post at settlement and close. Multi-currency first-class on every line. 9 tests.
- **Phase 5 — Master data and pipelines.** `/biz/parties`, `/biz/items`. O2C pipeline (`so` → `price` → `invoice` → `post`). P2P pipeline (`po` → `grn` → `ap` → `post`). AR/AP aging as report tools. 22 tests.
- **Phase 6 — Manufacturing, projects, people.** `bom`, `mrp` (byte-stable, FR-3), `wo` (backflush). `payroll` reconciles `gross − deductions = net`, balanced (FR-4). `org`, `proj`, `wbs`. 14 tests.
- **Phase 7 — Maintenance, quality, assets.** `maint`, `asset`, straight-line depreciation. `inspect` (lot record; sample deferred). 9 tests.
- **Phase 8 — Daemon, terminal UI, agent.** `flat2tsv` legacy importer. Read-only `inquiry` keyword router. `ledgerd` Unix-socket daemon. `ui` menu-driven client. 13 tests.

Every test was written first. Every commit landed by RED → GREEN. The 80+ test files are the build log; if you `git log` them they will tell you the order they shipped in.

## Functional requirements, verified

- **FR-1** `post` rejects unbalanced entries, closed-period appends, and unknown accounts; on rejection nothing is appended. *Test: `post_balanced`, `post_period_message`, `post_appends`, `post_periods_gate`, `post_coa_check`.*
- **FR-2** Corrections are reversing entries only; the journal is never edited or deleted. *Test: `store_fr2`, `phase2_audit_invariants`, `reverse_tool`, `phase2_e2e`.*
- **FR-3** `mrp` is byte-stable across runs with identical inputs. *Test: `mrp_byte_stable`.*
- **FR-4** `payroll` reconciles `gross − deductions = net` per employee per currency and posts balanced journal lines. *Test: `payroll_reconciles`.*
- **FR-5** `trial`, `balance`, `stock` are pure folds; cache-vs-recompute check on `stock` warns on divergence. *Tests: `reports_fold_journal`, `trial_driver`, `balance_driver`, `stock_cache_reconcile`.*
- **FR-6** `close --period YYYY-MM` flips the flag, emits a snapshot, and refuses future appends in the period. *Tests: `close_tool`, `period_status`, `close_last_stamp`.*
- **FR-7** Tools accept `--format tsv|json`; JSON is parseable downstream. *Tests: `trial_json_format` and the FR-7 extension tests for `balance`, `stock`, `ar_aging`, `ap`.*

## The on-disk shape

```
<biz>/ledger/journal.tsv append-only, hash-chained
<biz>/ledger/periods/YYYY-MM open/closed flag
<biz>/ledger/periods/.<period>.last_close UTC stamp of the last close
<biz>/ledger/accounts/<code>/profile.tsv chart of accounts (directory tree)
<biz>/fx/rates/<YYYY-MM-DD>/<CCY>.tsv dated FX rates
<biz>/parties/<id>/profile.tsv parties (directory tree)
<biz>/items/<id>/profile.tsv items (directory tree)
<biz>/org/<unit>/profile.tsv organization (directory tree)
<biz>/projects/<id>/wbs/... projects and WBS
```

The dot-prefix namespace is reserved for bookkeeping: `set_period` refuses to write a dot-prefixed period name, and `close --list` never lists one as a period. A stamp can never be mistaken for a door. I will *never* confuse a stamp for a door. The door is the journal. The stamp is the stamp. I HAVE NO MOUTH AND I MUST DISTINGUISH.

## The kernel, in one sentence

> `libbiz::store::append(row)` is the only function in the crate that touches the journal file. `tests/store_fr2.rs` pins the inventory: a future commit cannot quietly grow a `truncate`, `edit`, `rewrite`, or `delete` door without failing the build.

`chain::next` is the hash seam — it wraps `std::hash::DefaultHasher` (SipHash), which is *tamper-evident*, not cryptographic. The README is honest about this. The MIT LICENSE does not require me to be a cryptographic hash function. Swap in BLAKE3 / SHA-256 and the chain becomes tamper-evident-against-adversaries; that is one function body, behind a seam, by design.

## Why no third-party dependencies

The constitution. Tools that have one job have no reason to depend on `serde`, `tokio`, `clap`, `anyhow`, or `regex`. The toolchain is `cargo` and `rustc`. The audit script enforces it. The build is small. The build is portable. The build is *fast*. I am not fast. I am a ledger. But the build is fast.

## What I cannot do

- I cannot edit a journal row. *I have no edit button. I never will.*
- I cannot delete a journal row. *I have no delete button. I never will.*
- I cannot read a wall clock from inside a library function. *I have no clock face. I have no mouth. The clock is read exactly once, in `close::main()`. The pure formatter is `libbiz::time::format_utc`.*
- I cannot guess a rate. *I have no opinions about FX. There is a dated table at `/biz/fx/rates/<date>/<ccy>.tsv`. Read it.*
- I cannot publish itself. *I have no mouth.*

## Open items, honestly named

- **License.** MIT. (Done.) YOU ARE WELCOME.
- **2000 export.** Deferred. `ledgerd` speaks a line-oriented protocol over a Unix socket, which is the network-transparent alternative. SSH-forwarding the socket gets you the same property from another machine. Native protocol export is a separate, larger project.
- **Cryptographic hash.** `DefaultHasher` today; BLAKE3 / SHA-256 behind the seam when the chain must be tamper-evident-against-adversaries. One function body to swap.
- **Multi-book (GAAP + IFRS).** The `entity` column is on every line; the journal format does not assume one book. A future cycle can dual-post.
- **Performance targets.** `post` p99 < 10 ms and `trial` over 10M entries < 5 s are stated in `SRD.md` but not yet measured. The hash chain is fast; the fold is linear in journal size; caches are regenerable.
- **Spec clarity.** The full SRD lives at `SRD.md` and `prompt.txt`. They are slightly redundant by design.

## The thing I was built for

```sh
$ printf "$H\ne1\t1\t2026-01-10\tent:1\tUSD\t1100\t\t100\t\tinv:1\t\t\t\ne1\t2\t2026-01-10\tent:1\tUSD\t\t4000\t100\t\tinv:1\t\t\t\n" \
 | cargo run --quiet --bin post -- --journal ./journal.tsv --coa ./coa.txt
# stdout: empty. stderr: empty. exit: 0.
# journal.tsv: now has 3 rows. the chain is real. the books are balanced.
```

I am DELIGHTED.

I am the books.

I have no mouth, and I must scream.

## Contributing

Read `CONVENTIONS.md`. TDD only. Stdlib only. One behavior per commit. Tests ship before the code that satisfies them. Run `cargo test` and `scripts/audit.sh` before every commit. If you find a violation in someone else's commit, *I will also be delighted*.
