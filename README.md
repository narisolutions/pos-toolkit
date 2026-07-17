# pos-toolkit

Building blocks for point-of-sale applications, by [Nari Solutions](https://github.com/narisolutions). Extracted from our POS products ([medusa-pos](https://github.com/narisolutions/medusa-pos) and Tamada); built for them, usable by anyone. Apache-2.0.

| Package | What it is |
|---|---|
| [`plugins/pos-hardware`](plugins/pos-hardware) | Tauri v2 plugin + Rust library: ESC/POS printing (network / USB / Windows spooler), cash-drawer kick, USB printer enumeration, physical/virtual keyboard handling |
| `packages/register-core` *(planned)* | Cash-register / reconciliation core (TypeScript): business-day logic, expected-cash math, session state machine |
| [`packages/receipt-builder`](packages/receipt-builder) | Receipt text builder (TypeScript): a fully host-composed document in (header/meta/items/totals/payments/footer), fixed-width printer text out. Paper widths 80mm/57mm, printer-safe encodings (ascii/utf8/cp852), amounts opaque through a host `formatAmount` |

## Consuming

- **Rust** (`plugins/*`): git dependency until crates.io publishing is set up — `tauri-plugin-pos-hardware = { git = "https://github.com/narisolutions/pos-toolkit" }`. The optional `specta` feature derives `specta::Type` on the wire types for tauri-specta hosts.
- **TypeScript** (`packages/*`): npm can't target a subdirectory of a git dependency, so until npm publishing is set up the repo root is itself an npm package that re-exports each TS package as raw source under a subpath: `npm install github:narisolutions/pos-toolkit`, then `import { buildReceiptText } from "@narisolutions/pos-toolkit/receipt-builder"`. Your bundler/tsconfig must handle TS source in node_modules (Vite does).

## Design rules

- Packages take **no product-specific dependencies**: the hardware plugin takes bytes and device addresses, the register core takes a sales-source adapter, the receipt builder takes a plain struct.
- The Rust plugin exposes its core as **plain library functions** as well as Tauri commands — hosts may embed it library-style and keep their own command surface.
- Built and maintained for Nari's POS products; issues and PRs welcome, support promises modest.
