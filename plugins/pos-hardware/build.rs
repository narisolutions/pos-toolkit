const COMMANDS: &[&str] = &[
    "print_job",
    "open_cash_drawer",
    "list_usb_devices",
    "list_system_printers",
    "check_physical_keyboard",
    "toggle_virtual_keyboard",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
