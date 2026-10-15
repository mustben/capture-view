#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod capture;
mod layout;
mod window;
fn main() {
    app::main();
}
