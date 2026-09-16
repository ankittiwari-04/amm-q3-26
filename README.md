# AMM (Constant Product Market Maker) — Turbin3 Q3'26

An Anchor-based Automated Market Maker (AMM) on Solana, implementing the
classic constant-product (`x * y = k`) design, extended with a protocol
fee routed to a dedicated treasury account.

Program: `amm_video` — built with Anchor, tested with LiteSVM (in-process,
no local validator needed).

---

## What this program does

Users can:
1. **Initialize** a new liquidity pool for a pair of tokens (Mint X / Mint Y).
2. **Deposit** both tokens into the pool, receiving LP (liquidity provider) tokens in return.
3. **Withdraw** liquidity by burning LP tokens, receiving back a proportional share of the pool.
4. **Swap** one token for the other, paying a small fee — split between liquidity providers and the protocol treasury.

---

## Instructions

| Instruction | What it does |
|---|---|
| `initialize(seed, fee, authority)` | Creates a new `Config` account (the pool), the LP mint, two vault token accounts, and the treasury's fee-collection ATAs. |
| `deposit(amount, max_x, max_y)` | Deposits tokens into the pool's vaults, mints LP tokens to the depositor. |
| `withdraw(amount, min_x, min_y)` | Burns LP tokens, returns a proportional share of both vaults to the user. |
| `swap(is_x, amount_in, min_amount_out)` | Swaps one token for the other using the constant-product formula, with slippage protection via `min_amount_out`. |

---

## Fees & Treasury

The pool has a single overall swap `fee` (in basis points), which is
split into two parts:

- **LP fee** — stays inside the pool's vaults, growing the reserves.
  Every liquidity provider benefits proportionally to their share.
- **Protocol fee** — a configurable percentage (`protocol_fee`, in bps of
  the total fee) is routed to a separate **treasury** account, owned by a
  wallet specified at `initialize` time — completely separate from the
  pool's own vaults.

### Why a separate treasury, not just the pool authority's wallet?

The treasury is deliberately **not** the same account as the transaction
payer or pool authority's main wallet. If it were, the treasury's
Associated Token Accounts (ATAs) would collide with that wallet's normal
token accounts for the same mints — since an ATA address is
deterministically derived from `(owner, mint)`, one wallet can only ever
have one ATA per mint. Using a dedicated treasury keypair/address avoids
this collision entirely and keeps protocol revenue cleanly separated from
any other balances that wallet might hold.

### Config account fields

```rust
pub struct Config {
    pub seed: u64,                 // Distinguishes multiple pools/configs
    pub authority: Option<Pubkey>, // Optional authority that can lock the pool
    pub mint_x: Pubkey,
    pub mint_y: Pubkey,
    pub fee: u16,                  // Total swap fee, in basis points
    pub protocol_fee: u16,         // Share of `fee` routed to treasury (bps of fee)
    pub treasury: Pubkey,          // Wallet that owns the treasury ATAs
    pub locked: bool,
    pub config_bump: u8,
    pub lp_bump: u8,
}
```

---

## Tests

All tests run via LiteSVM — an in-process Solana VM, so no local
validator or `anchor test` is needed.

```bash
anchor build
cd programs/amm-video
cargo test -- --nocapture
```

**6 tests, all passing:**

| Test | What it verifies |
|---|---|
| `test_initialize` | Pool, LP mint, vaults, and treasury ATAs are created correctly. |
| `test_deposit` | Depositing tokens correctly mints LP tokens and fills the vaults. |
| `test_withdraw` | Burning LP tokens correctly returns a proportional share of both vaults. |
| `test_swap_with_slippage` | A normal swap executes correctly within slippage bounds. |
| `test_swap_slippage_exceeded` | A swap that would violate `min_amount_out` is correctly rejected. |
| `test_swap_routes_protocol_fee_to_treasury` | After a swap, the treasury's token balance has genuinely increased — proving the protocol fee is real, not just calculated. |

```
running 6 tests
test test_withdraw ... ok
test test_initialize ... ok
test test_swap_routes_protocol_fee_to_treasury ... ok
test test_deposit ... ok
test test_swap_slippage_exceeded ... ok
test test_swap_with_slippage ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

*(See screenshot in submission for the full terminal output.)*

---

## Extension challenges

- **Implement CPMM without the library** — *not attempted in this
  submission.* The current implementation uses the `constant_product_curve`
  crate for the `x * y = k` math and slippage handling.
- **Downtime mitigation for DeFi apps** — *not attempted in this
  submission.*

---

## Project structure

```
amm-q3-26/
├── programs/amm-video/
│   ├── src/
│   │   ├── lib.rs              # Program entrypoint, instruction routing
│   │   ├── state.rs             # Config account definition
│   │   ├── constants.rs
│   │   ├── error.rs
│   │   └── instructions/
│   │       ├── initialize.rs    # Pool + treasury ATA creation
│   │       ├── deposit.rs
│   │       ├── withdraw.rs
│   │       └── swap.rs          # Swap logic + protocol fee routing
│   └── tests/
│       ├── tests.rs              # LiteSVM test suite
│       └── ix_handlers/          # Per-instruction test helpers
├── Anchor.toml
└── Cargo.toml
```

---

## About

Built as part of the Turbin3 Q3'26 Builders Cohort.
GitHub: [@ankittiwari-04](https://github.com/ankittiwari-04)
