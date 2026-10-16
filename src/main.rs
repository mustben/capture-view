#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod capture;
mod dib;
mod graph_state;
mod layout;
mod selection;
mod window;
fn main() {
    app::main();
}
