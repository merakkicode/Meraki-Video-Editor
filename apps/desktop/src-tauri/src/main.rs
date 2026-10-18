#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]
fn main() {
    meraki_desktop_lib::run();
}
