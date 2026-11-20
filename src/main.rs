#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod map;
mod network;
mod sim;
mod ui;

use eframe::egui;
use network::Network;
use sim::Simulation;

fn main() -> eframe::Result {
    let network = Network::load().expect("Invalid bundled Vilnius road graph");
    let simulation = Simulation::new(network);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 820.0])
            .with_min_inner_size([800.0, 570.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Vilniaus evakuacijos simuliacija",
        options,
        Box::new(|context| Ok(Box::new(ui::DesktopApp::new(context, simulation)))),
    )
}
