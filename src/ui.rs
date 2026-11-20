use crate::{
    map::{ACCENT, CLOSED, MapClick, MapView},
    sim::{AGENT_COUNT, Simulation, Status},
};
use eframe::egui::{self, Color32, RichText};
use std::time::Duration;

const PAGE: Color32 = Color32::from_rgb(15, 25, 28);
const PANEL: Color32 = Color32::from_rgb(22, 34, 38);
const INK: Color32 = Color32::from_rgb(231, 241, 238);
const MUTED: Color32 = Color32::from_rgb(164, 187, 189);
const HINT: &str = "Pasirinkite agentą arba vilkite žemėlapį";
const CLOSURE_HINT: &str = "Pasirinkite kelio atkarpą";

pub struct DesktopApp {
    simulation: Simulation,
    map: MapView,
    playing: bool,
    closing: bool,
    speed: f64,
    selected: Option<usize>,
    agent_number: usize,
    hint: String,
}

impl DesktopApp {
    pub fn new(context: &eframe::CreationContext<'_>, simulation: Simulation) -> Self {
        let mut style = (*context.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = PANEL;
        style.visuals.window_fill = PAGE;
        style.visuals.override_text_color = Some(INK);
        style.visuals.selection.bg_fill = Color32::from_rgb(44, 95, 84);
        style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(34, 52, 57);
        style.spacing.item_spacing = egui::vec2(8.0, 9.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        context.egui_ctx.set_style(style);
        let map = MapView::new(simulation.network.bounds);
        Self {
            simulation,
            map,
            playing: false,
            closing: false,
            speed: 1.0,
            selected: None,
            agent_number: 1,
            hint: HINT.to_owned(),
        }
    }

    fn toggle_playback(&mut self) {
        if !self.playing && self.simulation.counts().traveling == 0 {
            self.reset();
        }
        self.playing = !self.playing;
    }

    fn reset(&mut self) {
        self.simulation.reset();
        self.playing = false;
        self.closing = false;
        self.speed = 1.0;
        self.selected = None;
        self.agent_number = 1;
        self.hint = HINT.to_owned();
    }

    fn select_agent(&mut self, selected: Option<usize>) {
        self.selected = selected;
        if let Some(index) = selected {
            self.agent_number = index + 1;
            self.hint = format!("Pasirinktas agentas {}", index + 1);
        } else {
            self.hint = HINT.to_owned();
        }
    }

    fn change_road(&mut self, road: usize, closed: bool) {
        self.simulation.set_closed(road, closed);
        let verb = if closed { "Uždaryta" } else { "Atverta" };
        self.hint = format!("{verb} · {}", self.simulation.network.road_name(road));
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let counts = self.simulation.counts();
        let seconds = self.simulation.elapsed as u64;
        ui.horizontal(|ui| {
            ui.heading("Simuliacija");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{:02}:{:02}", seconds / 60, seconds % 60))
                        .monospace()
                        .color(ACCENT),
                );
            });
        });
        ui.horizontal(|ui| {
            let label = if self.playing {
                "Pristabdyti"
            } else if counts.traveling == 0 {
                "Kartoti"
            } else {
                "Paleisti"
            };
            let play = egui::Button::new(RichText::new(label).strong().color(PAGE)).fill(ACCENT);
            if ui
                .add_sized([ui.available_width() - 75.0, 32.0], play)
                .clicked()
            {
                self.toggle_playback();
            }
            if ui.button("Atkurti").clicked() {
                self.reset();
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Greitis").color(MUTED));
            for (speed, label) in [(1.0, "1×"), (2.0, "2×"), (4.0, "4×")] {
                ui.selectable_value(&mut self.speed, speed, label);
            }
        });

        ui.separator();
        ui.heading("Agentų eiga");
        ui.add(egui::ProgressBar::new(counts.arrived as f32 / AGENT_COUNT as f32).fill(ACCENT));
        for (text, value, color) in [
            ("Kelyje", counts.traveling, INK),
            ("Pasiekė tikslą", counts.arrived, ACCENT),
            ("Be maršruto", counts.stranded, CLOSED),
        ] {
            ui.horizontal(|ui| {
                ui.label(RichText::new(text).color(MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(value.to_string())
                            .strong()
                            .monospace()
                            .color(color),
                    );
                });
            });
        }

        ui.separator();
        ui.heading("Kelių uždarymai");
        let label = if self.closing {
            "Baigti pasirinkimą"
        } else {
            "Pasirinkti kelio atkarpą"
        };
        if ui
            .add_sized([ui.available_width(), 30.0], egui::Button::new(label))
            .clicked()
        {
            self.closing = !self.closing;
            self.hint = if self.closing { CLOSURE_HINT } else { HINT }.to_owned();
        }
        ui.label(
            RichText::new("Atkarpą taip pat galima pasirinkti dešiniuoju pelės mygtuku.")
                .color(MUTED),
        );
        let mut reopen = None;
        for &road in self.simulation.closed_roads().iter().rev() {
            ui.horizontal_wrapped(|ui| {
                if ui.small_button("Atverti").clicked() {
                    reopen = Some(road);
                }
                ui.label(self.simulation.network.road_name(road));
            });
        }
        if let Some(road) = reopen {
            self.change_road(road, false);
        }

        ui.separator();
        ui.heading("Agento maršrutas");
        ui.horizontal(|ui| {
            ui.label("Nr.");
            ui.add(egui::DragValue::new(&mut self.agent_number).range(1..=AGENT_COUNT));
            if ui.button("Rodyti").clicked() {
                self.closing = false;
                let index = self.agent_number - 1;
                self.select_agent(Some(index));
                self.map.focus(self.simulation.position(index));
            }
            if self.selected.is_some() && ui.small_button("Išvalyti").clicked() {
                self.select_agent(None);
            }
        });
        if let Some(index) = self.selected {
            let route = self.simulation.route(index);
            let status = match (route.returning, self.simulation.status(index)) {
                (true, _) => "Grįžta į sankryžą",
                (_, Status::Traveling) => "Turi maršrutą",
                (_, Status::Arrived) => "Pasiekė tikslą",
                (_, Status::Stranded) => "Be maršruto",
            };
            ui.label(RichText::new(status).color(ACCENT));
            if let Some(target) = route.destination {
                ui.label(format!("Tikslas {target}"));
            }
            if let Some(meters) = route.remaining_meters {
                ui.label(if meters < 1000.0 {
                    format!("Liko {meters:.0} m")
                } else {
                    format!("Liko {:.1} km", meters / 1000.0).replace('.', ",")
                });
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("+").on_hover_text("Priartinti").clicked() {
                self.map.zoom(1.35);
            }
            if ui.button("−").on_hover_text("Atitolinti").clicked() {
                self.map.zoom(1.0 / 1.35);
            }
            if ui.button("Visas miestas").clicked() {
                self.map.fit(self.simulation.network.bounds);
            }
        });
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !ctx.wants_keyboard_input() {
            if ctx.input(|input| input.key_pressed(egui::Key::Space)) {
                self.toggle_playback();
            }
            if ctx.input(|input| input.key_pressed(egui::Key::R)) {
                self.reset();
            }
            if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
                self.closing = false;
                self.select_agent(None);
            }
        }
        if self.playing {
            let dt = ctx.input(|input| f64::from(input.stable_dt.clamp(0.0, 0.05)));
            self.simulation.step(dt * self.speed);
            self.playing = self.simulation.counts().traveling > 0;
            if self.playing {
                ctx.request_repaint_after(Duration::from_millis(16));
            }
        }
        egui::TopBottomPanel::top("title")
            .frame(egui::Frame::new().fill(PAGE).inner_margin(14))
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("HAKATONO PROTOTIPAS")
                        .size(11.0)
                        .strong()
                        .color(ACCENT),
                );
                ui.label(
                    RichText::new("Vilniaus evakuacijos simuliacija")
                        .size(26.0)
                        .strong(),
                );
            });
        egui::TopBottomPanel::bottom("attribution").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.hyperlink_to(
                    "© OpenStreetMap contributors",
                    "https://www.openstreetmap.org/copyright",
                );
                ui.label(
                    RichText::new(format!(
                        "· {} · Pavyzdiniai tikslai ir sutartinis judėjimo greitis",
                        self.simulation.network.source_date
                    ))
                    .small()
                    .color(MUTED),
                );
                ui.hyperlink_to(
                    "Vilniaus civilinė sauga",
                    "https://vilnius.lt/savivaldybe/saugus-miestas/civiline-sauga",
                );
            });
        });
        egui::SidePanel::right("controls")
            .resizable(false)
            .exact_width(295.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.controls(ui));
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(PAGE).inner_margin(12))
            .show(ctx, |ui| {
                if let Some(click) = self.map.show(
                    ui,
                    &self.simulation,
                    self.selected,
                    self.closing,
                    &self.hint,
                ) {
                    match click {
                        MapClick::Agent(selected) => self.select_agent(selected),
                        MapClick::Road(road) => {
                            self.change_road(road, !self.simulation.closed_roads().contains(&road))
                        }
                        MapClick::MissingRoad => {
                            self.hint = "Priartinkite žemėlapį ir pasirinkite atkarpą".to_owned()
                        }
                    }
                    ctx.request_repaint();
                }
            });
    }
}
