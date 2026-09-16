//! USB HID POS Scales (HID usage page 0x8D, the "Point of Sale" page).
//!
//! ## Why this is one driver and not a vendor table
//!
//! The USB-IF's *HID Point of Sale Usage Tables* define a **Scale Data
//! Report** that every class-compliant retail scale sends the same way:
//! a status byte (stable, in motion, zero, under zero, over capacity …), a
//! unit byte (g, kg, oz, lb, …), a signed base-10 exponent and a 16-bit
//! weight. Mettler Toledo Ariva, Datalogic Magellan, Zebra MP7000 and the
//! small Dymo/Brecknell counter scales all speak it. So, unlike bank ECR
//! protocols, a single generic reader is the honest answer, and the serial
//! families (Dialog 06, NCI) can come later as their own modules if a venue
//! turns up with one.
//!
//! ## What this module does not decide
//!
//! Nothing here prices anything, converts a unit, or decides whether the
//! reading may be sold. It reports **what the scale said, as it said it** —
//! the status word, the unit word, the amount in thousandths of that unit —
//! and the host owns the rest: which unit the product sells by, whether a
//! reading in motion may be booked, and what its market's law says about a
//! POS pricing a counter sale from a scale it read (that last one is the
//! host's compliance notice, not this module's business).
//!
//! ## The amount is an integer
//!
//! `value_milli` is thousandths of `unit`, never a float: `w × 10^(exp+3)`
//! computed in integers. A scale reporting finer than a thousandth of its
//! unit (an exponent below −3) is rounded half-up to the thousandth, and
//! the raw report rides every event so nothing is lost for a bench.
//!
//! ## Platform notes
//!
//! - **Linux:** the device is a `/dev/hidraw*` node. Stock udev leaves those
//!   root-only, so the first open on a new machine usually fails with a
//!   permission error; [`open_error_hint`] turns that into the udev rule that
//!   fixes it. The kernel's generic HID driver owns the device and hidraw
//!   reads beside it — nothing is detached and no other driver is displaced.
//! - **Windows / macOS:** the native HID stacks, no driver install.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};

#[cfg(feature = "specta")]
use specta::Type;

/// The HID usage page every POS scale reports on.
pub const POS_USAGE_PAGE: u16 = 0x8D;

/// The event a reading is emitted on.
pub const READING_EVENT: &str = "pos-hardware://scale-reading";

/// The event a reader emits when it stops on an error rather than on request.
pub const ERROR_EVENT: &str = "pos-hardware://scale-error";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HidScaleInfo {
    /// What to pass back to [`open_scale`]. `/dev/hidraw3`, a Windows device
    /// path, an IOKit path on macOS — opaque, and not stable across a re-plug
    /// on Linux (hidraw numbers enumerate in order). Store `serial_number`
    /// beside it where the scale has one and re-resolve.
    pub path: String,
    pub vid: u16,
    pub pid: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
    /// Reports on the POS usage page — a scale, as far as HID can tell. A
    /// picker shows these first; a keyboard or a mouse is never one.
    pub likely: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ScaleReading {
    /// Which device, so a host with two scales can tell them apart.
    pub path: String,
    /// `fault`, `zero`, `motion`, `stable`, `under_zero`, `over_capacity`,
    /// `calibration`, `rezero`, or `unknown` — the Scale Status usages
    /// (0x8D:0x70–0x78) by name. Only `stable` is a weight a host should book.
    pub status: String,
    /// `mg`, `g`, `kg`, `ct`, `tael`, `gr`, `dwt`, `t`, `ton`, `ozt`, `oz`,
    /// `lb`, or `unknown` — the Weight Unit usages (0x8D:0x11–0x1C) by name.
    pub unit: String,
    /// Thousandths of `unit`, as an integer. 352 on a kilogram scale is
    /// 0.352 kg. Zero when the status is anything but a weight.
    pub value_milli: i64,
    /// Exactly what arrived, for a bench.
    pub raw: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ScaleError {
    pub path: String,
    pub message: String,
}

/// Open readers, keyed by device path. Each holds the flag that stops its thread.
#[derive(Default)]
pub struct ScaleState(Mutex<HashMap<String, Arc<AtomicBool>>>);

/// HID devices the system knows about, POS scales first.
///
/// Every HID device is returned with `likely` saying which report on the POS
/// usage page, for the same reason the serial list returns every port: a
/// scale from a vendor no table has heard of must still be pickable, and a
/// human choosing from a list beats a table being wrong the first time.
pub fn list_hid_scales() -> Result<Vec<HidScaleInfo>, String> {
    let api = hidapi::HidApi::new().map_err(|e| format!("could not open the HID subsystem: {e}"))?;
    let mut out: Vec<HidScaleInfo> = api
        .device_list()
        .map(|d| HidScaleInfo {
            path: d.path().to_string_lossy().into_owned(),
            vid: d.vendor_id(),
            pid: d.product_id(),
            manufacturer: d.manufacturer_string().map(str::to_string),
            product: d.product_string().map(str::to_string),
            serial_number: d.serial_number().map(str::to_string),
            likely: d.usage_page() == POS_USAGE_PAGE,
        })
        .collect();
    // One row per device path — hidapi lists a device once per interface it
    // exposes, and a scale with a keyboard interface would appear twice.
    out.sort_by(|a, b| b.likely.cmp(&a.likely).then_with(|| a.path.cmp(&b.path)));
    out.dedup_by(|a, b| a.path == b.path);
    Ok(out)
}

/// The sentence that fixes the commonest failure, instead of the error alone.
pub fn open_error_hint(path: &str, err: &hidapi::HidError) -> String {
    let base = format!("could not open {path}: {err}");
    let text = err.to_string().to_ascii_lowercase();
    if cfg!(target_os = "linux") && (text.contains("permission") || text.contains("denied") || text.contains("access")) {
        return format!(
            "{base}. On Linux a HID device is root-only until udev says otherwise: add a rule such as \
             `SUBSYSTEM==\"hidraw\", ATTRS{{idVendor}}==\"<vid>\", MODE=\"0660\", GROUP=\"plugdev\"` \
             (or TAG+=\"uaccess\" on a desktop) to /etc/udev/rules.d/, reload udev, and re-plug the scale."
        );
    }
    base
}

fn status_name(code: u8) -> &'static str {
    match code {
        1 => "fault",
        2 => "zero",
        3 => "motion",
        4 => "stable",
        5 => "under_zero",
        6 => "over_capacity",
        7 => "calibration",
        8 => "rezero",
        _ => "unknown",
    }
}

fn unit_name(code: u8) -> &'static str {
    match code {
        1 => "mg",
        2 => "g",
        3 => "kg",
        4 => "ct",
        5 => "tael",
        6 => "gr",
        7 => "dwt",
        8 => "t",
        9 => "ton",
        10 => "ozt",
        11 => "oz",
        12 => "lb",
        _ => "unknown",
    }
}

/// `w × 10^(exp+3)`, in integers, half-up below the thousandth.
fn to_milli(weight: u16, exponent: i8) -> i64 {
    let w = i64::from(weight);
    let shift = i32::from(exponent) + 3;
    if shift >= 0 {
        w.saturating_mul(10_i64.pow(shift as u32))
    } else {
        let divisor = 10_i64.pow((-shift) as u32);
        (w + divisor / 2) / divisor
    }
}

/// Decode a Scale Data Report (report ID 3): `[3, status, unit, exp, lo, hi]`.
///
/// Some stacks hand the report without its ID byte; both shapes are read.
/// Anything else — an attribute report, a control report, a truncated read —
/// is `None`, and the reader simply waits for the next one.
pub fn parse_report(path: &str, raw: &[u8]) -> Option<ScaleReading> {
    let body: &[u8] = match raw {
        [3, rest @ ..] if rest.len() >= 5 => rest,
        [_, _, _, _, _] => raw,
        _ => return None,
    };
    let status = status_name(body[0]);
    let unit = unit_name(body[1]);
    let weight = u16::from_le_bytes([body[3], body[4]]);
    let value_milli = if status == "stable" || status == "motion" { to_milli(weight, body[2] as i8) } else { 0 };
    Some(ScaleReading { path: path.to_string(), status: status.to_string(), unit: unit.to_string(), value_milli, raw: raw.to_vec() })
}

/// Attach a reader; readings arrive on [`READING_EVENT`] whenever the scale's
/// report changes, and at least every `heartbeat_ms` while it does not — a
/// host drawing a live number must be able to tell "still 0.352 kg" from "the
/// scale went quiet".
pub fn open_scale<R: Runtime>(app: AppHandle<R>, state: &ScaleState, path: String, heartbeat_ms: u64) -> Result<(), String> {
    {
        let open = state.0.lock().map_err(|_| "scale state poisoned".to_string())?;
        if open.contains_key(&path) {
            return Ok(());
        }
    }
    let api = hidapi::HidApi::new().map_err(|e| format!("could not open the HID subsystem: {e}"))?;
    let device = api.open_path(&std::ffi::CString::new(path.clone()).map_err(|e| e.to_string())?).map_err(|e| open_error_hint(&path, &e))?;

    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut open = state.0.lock().map_err(|_| "scale state poisoned".to_string())?;
        open.insert(path.clone(), Arc::clone(&stop));
    }
    let thread_path = path.clone();
    std::thread::Builder::new()
        .name(format!("pos-scale:{path}"))
        .spawn(move || read_loop(app, device, thread_path, heartbeat_ms, stop))
        .map_err(|e| format!("could not start reader for {path}: {e}"))?;
    Ok(())
}

fn read_loop<R: Runtime>(app: AppHandle<R>, device: hidapi::HidDevice, path: String, heartbeat_ms: u64, stop: Arc<AtomicBool>) {
    let mut buf = [0u8; 64];
    let mut last: Option<(String, String, i64)> = None;
    let mut last_emit = std::time::Instant::now();
    let heartbeat = Duration::from_millis(heartbeat_ms.max(50));
    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        match device.read_timeout(&mut buf, 100) {
            Ok(0) => {}
            Ok(n) => {
                if let Some(reading) = parse_report(&path, &buf[..n]) {
                    let key = (reading.status.clone(), reading.unit.clone(), reading.value_milli);
                    let changed = last.as_ref() != Some(&key);
                    if changed || last_emit.elapsed() >= heartbeat {
                        if let Err(e) = app.emit(READING_EVENT, reading) {
                            log::warn!("scale reading emit failed on {path}: {e}");
                        }
                        last = Some(key);
                        last_emit = std::time::Instant::now();
                    }
                }
            }
            Err(e) => {
                let _ = app.emit(ERROR_EVENT, ScaleError { path: path.clone(), message: format!("the scale stopped answering: {e}") });
                return;
            }
        }
    }
}

pub fn close_scale(state: &ScaleState, path: &str) -> Result<(), String> {
    let mut open = state.0.lock().map_err(|_| "scale state poisoned".to_string())?;
    if let Some(stop) = open.remove(path) {
        stop.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stable_kilogram_report_reads_in_thousandths() {
        // status 4 stable, unit 3 kg, exponent -3, weight 352 → 0.352 kg
        let r = parse_report("dev", &[3, 4, 3, 0xFD, 0x60, 0x01]).unwrap();
        assert_eq!((r.status.as_str(), r.unit.as_str(), r.value_milli), ("stable", "kg", 352));
    }

    #[test]
    fn exponents_scale_in_integers_and_round_half_up_below_the_thousandth() {
        assert_eq!(to_milli(352, -3), 352);
        assert_eq!(to_milli(35, -2), 350);
        assert_eq!(to_milli(2, 0), 2000);
        assert_eq!(to_milli(3525, -4), 353);
        assert_eq!(to_milli(3524, -4), 352);
    }

    #[test]
    fn a_report_without_its_id_byte_and_a_pound_scale_both_read() {
        // Magellan-style: no leading ID; unit 12 lb, exponent -2, weight 125 → 1.250 lb
        let r = parse_report("dev", &[4, 12, 0xFE, 125, 0]).unwrap();
        assert_eq!((r.unit.as_str(), r.value_milli), ("lb", 1250));
    }

    #[test]
    fn motion_carries_its_number_and_the_rest_carry_zero() {
        assert_eq!(parse_report("dev", &[3, 3, 3, 0xFD, 10, 0]).unwrap().value_milli, 10);
        assert_eq!(parse_report("dev", &[3, 2, 3, 0xFD, 10, 0]).unwrap().value_milli, 0);
        assert_eq!(parse_report("dev", &[3, 6, 3, 0xFD, 10, 0]).unwrap().status, "over_capacity");
    }

    #[test]
    fn other_reports_are_ignored() {
        assert!(parse_report("dev", &[1, 2, 3]).is_none());
        assert!(parse_report("dev", &[]).is_none());
        assert_eq!(parse_report("dev", &[3, 4, 99, 0, 1, 0]).unwrap().unit, "unknown");
    }
}
