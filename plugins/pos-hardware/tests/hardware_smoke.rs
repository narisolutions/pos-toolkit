//! Real-hardware smoke tests — `#[ignore]`d so CI never needs a printer.
//! Point PRINTER_HOST (and optionally PRINTER_PORT) at a network ESC/POS
//! printer and run explicitly:
//!
//! ```sh
//! PRINTER_HOST=192.168.1.153 cargo test -p tauri-plugin-pos-hardware \
//!     --test hardware_smoke -- --ignored --nocapture
//! ```
//!
//! `print_test_page` uses one short strip of paper and exercises init, bold,
//! size multipliers, ASCII punctuation, and a UTF-8 Georgian line (whether it
//! renders is a firmware fact worth knowing per venue). `kick_drawer` sends
//! the bare pin-2 pulse — the drawer should pop, no paper should move.

use tauri_plugin_pos_hardware::printing::{open_cash_drawer, print_job, Align, PrintOp, PrinterTarget};

fn target() -> PrinterTarget {
    let host = std::env::var("PRINTER_HOST").expect("set PRINTER_HOST to run hardware smoke tests");
    let port = std::env::var("PRINTER_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(9100);
    PrinterTarget::Network { host, port }
}

fn text(text: &str) -> PrintOp {
    PrintOp::Text { text: text.into(), bold: false, align: None, size: None }
}

#[test]
#[ignore = "needs a real printer (PRINTER_HOST)"]
fn print_test_page() {
    let ops = vec![
        PrintOp::Text {
            text: "POS-TOOLKIT TEST".into(),
            bold: true,
            align: Some(Align::Center),
            size: None,
        },
        PrintOp::Text {
            text: "BIG 2x2".into(),
            bold: true,
            align: Some(Align::Center),
            size: Some((2, 2)),
        },
        text("------------------------------------------------"),
        text("ascii + punctuation: ()*%#@!?0123456789"),
        PrintOp::Text { text: "tall 1x2 line".into(), bold: true, align: None, size: Some((1, 2)) },
        text("utf8 georgian: \u{10ee}\u{10d8}\u{10dc}\u{10d9}\u{10d0}\u{10da}\u{10d8}"),
        text("(garbled/blank above = no UTF-8 firmware)"),
        text("------------------------------------------------"),
        PrintOp::Feed { lines: 3 },
        PrintOp::Cut,
    ];
    print_job(&target(), &ops).expect("print_job against the real printer");
}

#[test]
#[ignore = "needs a real printer with a drawer (PRINTER_HOST)"]
fn kick_drawer() {
    open_cash_drawer(&target()).expect("drawer kick against the real printer");
}
