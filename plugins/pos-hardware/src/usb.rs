//! USB printer enumeration (extracted from medusa-pos).

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub description: String,
}

/// Devices that look like printers: USB class 7, or composite devices with a
/// printer interface.
pub fn list_usb_devices() -> Result<Vec<UsbDeviceInfo>, String> {
    let devices = rusb::devices().map_err(|e| format!("Failed to enumerate USB devices: {e}"))?;
    let mut result = Vec::new();

    for device in devices.iter() {
        let Ok(desc) = device.device_descriptor() else { continue };

        let is_printer_class = desc.class_code() == 7;
        let is_composite = desc.class_code() == 0;
        let has_printer_interface = is_composite
            && device
                .active_config_descriptor()
                .map(|config| {
                    config
                        .interfaces()
                        .any(|iface| iface.descriptors().any(|id| id.class_code() == 7))
                })
                .unwrap_or(false);

        if !is_printer_class && !has_printer_interface {
            continue;
        }

        let handle = device.open().ok();
        let manufacturer = handle
            .as_ref()
            .and_then(|h| h.read_manufacturer_string_ascii(&desc).ok())
            .unwrap_or_default();
        let product = handle
            .as_ref()
            .and_then(|h| h.read_product_string_ascii(&desc).ok())
            .unwrap_or_default();

        let description = match (manufacturer.is_empty(), product.is_empty()) {
            (false, false) => format!("{manufacturer} {product}"),
            (true, false) => product,
            (false, true) => manufacturer,
            (true, true) => format!("USB Printer ({:04x}:{:04x})", desc.vendor_id(), desc.product_id()),
        };

        result.push(UsbDeviceInfo {
            vendor_id: desc.vendor_id(),
            product_id: desc.product_id(),
            description,
        });
    }

    Ok(result)
}
