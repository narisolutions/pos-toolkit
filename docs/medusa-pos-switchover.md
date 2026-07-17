# medusa-pos → pos-toolkit switchover

**Goal:** medusa-pos consumes the shared packages from this repo instead of its in-tree copies, so the hardware layer and receipt builder have one home ([narisolutions/pos-toolkit](https://github.com/narisolutions/pos-toolkit), Apache-2.0). Tamada already consumes both (since 2026-07-16/17); this is the medusa-pos side, shippable as a normal release.

Part 1 (the Rust hardware layer) is the load-bearing dedup — it removes a security-sensitive layer that currently exists twice. Part 2 (receipt builder) is smaller and can ship in the same release or a later one.

---

## Part 1 — replace the in-tree Rust hardware layer with `tauri-plugin-pos-hardware`

### What the plugin provides

Extracted from medusa-pos `src-tauri` (same `escpos` 0.19 + `rusb`, same transports), so behavior parity is by construction:

- `printing::print_job(&PrinterTarget, &[PrintOp])` — one connection per job; ops are `Text { text, bold, align, size }`, `Image { path, max_width }`, `Feed`, `Cut`, `DrawerKick`, `Raw { bytes }`.
- `printing::open_cash_drawer(&PrinterTarget)` — byte-identical to the in-tree version (network sends the bare `1B 70 00 19 FF` pulse with no init; USB/spooler init + Pin2).
- `usb::list_usb_devices()` — the class-7 / composite-with-printer-interface scan, unchanged.
- `winprint::{raw_print, list_system_printers}` — the Windows spooler layer, unchanged (⚠ one wire change, see below).
- `keyboard::{has_physical_keyboard, toggle_virtual_keyboard}` — unchanged.

CI here runs clippy `-D warnings` + tests, including integration tests that drive the network path against a local TCP listener (init/bold/size/cut bytes; drawer pulse) and wire-format tests pinning the ops JSON.

### Recommended shape: library-style, keep your command names

The plugin exposes everything as **plain library functions**, so medusa-pos keeps its existing command surface (`print_receipt`, `print_test`, `open_cash_drawer`, `list_usb_devices`, `list_system_printers`, keyboard commands) and the frontend needs (almost) no changes. Don't register the Tauri plugin (`init()`) — that would add a second, differently-named command set.

1. **Cargo.toml** — add the dep, drop what moves:

   ```toml
   tauri-plugin-pos-hardware = { git = "https://github.com/narisolutions/pos-toolkit", branch = "main" }
   ```

   Then remove `escpos`, `rusb`, and the `windows` target-dep **if** nothing else in medusa-pos uses them (check `appimage_integrate.rs` / `config.rs` first). Do **not** enable the `specta` feature — that's for tauri-specta hosts (Tamada).

2. **Delete from `src-tauri/src/`**: `keyboard.rs`, `winprint.rs`, and in `lib.rs`: `BufferDriver` + `create_buffer_printer`, `create_network_printer`, `create_usb_printer`, `map_printer_error`, `configure_printer_for_georgian` (it was log-only — UTF-8 pass-through is what the plugin does anyway), the `list_usb_devices` body, and the bodies of `open_cash_drawer` / `print_receipt` / `print_test`.

3. **Rewrite the command bodies as composition + delegation.** The pattern:

   ```rust
   use tauri_plugin_pos_hardware::{keyboard, printing, usb, winprint};
   use printing::{Align, PrintOp, PrinterTarget};

   fn target(connection_type: &str, address: &str, port: Option<String>,
             vendor_id: Option<u16>, product_id: Option<u16>) -> Result<PrinterTarget, String> {
       match connection_type {
           "network" => Ok(PrinterTarget::Network { host: address.into(), port: parse_port(port) }),
           "usb" => { let (vid, pid) = parse_usb_ids(vendor_id, product_id)?;
                      Ok(PrinterTarget::Usb { vendor_id: vid, product_id: pid }) }
           "local" => Ok(PrinterTarget::System { name: address.into() }),
           _ => Err("Unsupported connection type".into()),
       }
   }
   ```

   `print_receipt` becomes: resolve logo path (that stays medusa-side — `get_logo_path`, `save_logo_file`, etc. are app concerns), then

   ```rust
   let mut ops = Vec::new();
   match get_logo_path(&app_handle) {
       Ok(path) => ops.push(PrintOp::Image { path, max_width: Some(384) }),
       Err(_) => ops.push(PrintOp::Text { text: header.into(), bold: true, align: Some(Align::Center), size: None }),
   }
   ops.push(PrintOp::Feed { lines: 1 });
   ops.push(PrintOp::Text { text: receipt_data, bold: false, align: None, size: None });
   ops.push(PrintOp::Feed { lines: 2 });
   ops.push(PrintOp::Cut);
   printing::print_job(&target, &ops)
   ```

   ⚠ **Logo fallback ladder:** the old `print_receipt_page` retried the logo through six `BitImageOption` configs (384/256/192/unconstrained/double-width/double-height) before falling back to the text header. `PrintOp::Image` is one attempt at `Normal` size with an optional `max_width`. If you want to keep the ladder, catch the `print_job` error and retry with a smaller `max_width` / text header (whole-job retry is fine — a failed image op errors before paper moves in practice, and reconnect-per-job survives it). If the ladder was only ever band-aid for one printer, this is the moment to simplify to `384 → text header`.

   `print_test` is the same composition with the test-page lines. `open_cash_drawer` / `list_usb_devices` / `list_system_printers` / keyboard commands become one-line delegations.

4. **⚠ One frontend-visible wire change:** `SystemPrinterInfo` now serializes **camelCase** (`driverName`, `portName`, `isDefault`) like the plugin's other types — the in-tree version was snake_case. Update:
   - `src/components/settings/printer/dialog/hooks.ts` — the `SystemPrinter` type (`driver_name`/`port_name`/`is_default` fields);
   - `src/components/settings/printer/dialog/index.tsx` — the `printer.port_name` / `printer.is_default` reads (~lines 211–212).

   Everything else on the frontend (`usePrinterService`, printer settings storage) keeps working unchanged because the command names and argument shapes stay yours.

### Verification checklist (before the release)

- `cargo clippy -- -D warnings` + build on Windows specifically (the spooler path and keyboard detection are Windows-only code).
- Physical pass with the settings dialog: list system printers (Windows), list USB devices, test page + receipt on **network, USB, and spooler** transports, drawer kick on each, logo prints (and the text-header fallback when the logo file is absent), a Georgian receipt on the UTF-8-capable printer, virtual-keyboard toggle on a touch device.
- `Cargo.lock` pins the git dep to a rev — commit it; bump deliberately with `cargo update -p tauri-plugin-pos-hardware`.

---

## Part 2 — adopt `packages/receipt-builder` (optional, can trail)

The shared builder is medusa's `src/utils/pos/receipt/` **generalized**: instead of a Medusa-flavored `ReceiptData`, it takes a fully host-composed `ReceiptDoc` — you decide which rows exist and hand over strings + numbers:

```ts
import { buildReceiptText, type ReceiptDoc } from "@narisolutions/pos-toolkit/receipt-builder";

const doc: ReceiptDoc = {
  headerLines: [companyName, storeName, storeAddress, `Tel: ${phone}`],
  title: t("receipt.title"),
  metaRows: [{ label: t("receipt.date"), value: dateStr }, { label: t("receipt.order"), value: `#${displayId}` }],
  itemsHeading: t("receipt.items"),
  items: [{ title, qty, unitPrice, total, sublines: [{ text: t("receipt.discount"), amount: -disc }] }],
  totalRows: [ /* subtotal/discount/VAT/total — you decide which exist */ ],
  paymentRows: [ /* payment method, amount paid, change, rounding */ ],
  messages: isUnpaid ? [t("receipt.unpaid")] : [],
  footerLines: [footer ?? t("receipt.thank_you")],
};
const text = buildReceiptText(doc, { formatAmount: (n) => formatCurrencyRaw(n, currency), paperWidth, encoding });
```

- **Install:** `@narisolutions/pos-toolkit` as a `github:narisolutions/pos-toolkit` dependency. npm can't target a subdirectory of a git dep, so the repo root re-exports the package as a **raw TypeScript source** subpath — Vite handles `.ts` in node_modules; no build step. (Moves to a normal npm package once registry publishing is set up.)
- **What maps where:** `buildReceiptDataFromOrder` keeps doing the Medusa-specific math (metadata discounts, cash rounding, pay-later detection) but now ends by composing the `ReceiptDoc` above instead of a `ReceiptData`; the `ReceiptLabels` object dissolves into the rows (i18n stays yours, via `t`). Amounts stay whatever you pass — the builder never formats, your `formatAmount` does.
- **Deliberate behavior change:** the `ascii` sanitizer keeps printable ASCII punctuation now — the in-tree one stripped `()`, `*`, `%` etc. from item titles. Receipts get slightly more faithful titles; nothing else changes (`utf8`/`cp852` behave the same, cp852's map is embedded so `printer-profiles.json` loses its `map` blob).
- **Stays medusa-side:** `buildReceiptPDF` (jspdf) — PDF export is host UI, not shared.
- Delete `src/utils/pos/receipt/` once callers are moved (`usePrinterService`, and the checkout/order/register hooks that import `ReceiptData`).

---

Questions / API friction: the toolkit is ours — if the plugin's op model is missing something medusa needs (e.g. an image-size option for the logo ladder), open an issue or PR here rather than keeping a fork in-tree.
