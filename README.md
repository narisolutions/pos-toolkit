# pos-toolkit

Building blocks for point-of-sale applications, by [Nari Solutions](https://github.com/narisolutions). Extracted from our POS products ([medusa-pos](https://github.com/narisolutions/medusa-pos) and Tamada); built for them, usable by anyone. Apache-2.0.

| Package | What it is |
|---|---|
| [`plugins/pos-hardware`](plugins/pos-hardware) | Tauri v2 plugin + Rust library: ESC/POS printing (network / USB / Windows spooler), cash-drawer kick, USB printer enumeration, physical/virtual keyboard handling |
| `packages/register-core` *(planned)* | Cash-register / reconciliation core (TypeScript): business-day logic, expected-cash math, session state machine |
| `packages/receipt-builder` *(planned)* | Receipt text builder (TypeScript): plain struct in, printer-ready output |

## Design rules

- Packages take **no product-specific dependencies**: the hardware plugin takes bytes and device addresses, the register core takes a sales-source adapter, the receipt builder takes a plain struct.
- The Rust plugin exposes its core as **plain library functions** as well as Tauri commands — hosts may embed it library-style and keep their own command surface.
- Built and maintained for Nari's POS products; issues and PRs welcome, support promises modest.
