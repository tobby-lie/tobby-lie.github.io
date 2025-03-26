use crate::components::app::App;

pub mod components;

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    log::info!("Launching app!");
    dioxus::launch(App);
}
