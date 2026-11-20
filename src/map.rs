use crate::{
    network::{Bounds, Point},
    sim::{AGENT_COUNT, Simulation, Status, TARGET_LABELS},
};
use eframe::egui::{self, Color32, FontId, Mesh, Pos2, Rect, Shape, Stroke, Vec2};
use std::sync::Arc;

pub const ACCENT: Color32 = Color32::from_rgb(121, 224, 197);
pub const CLOSED: Color32 = Color32::from_rgb(255, 144, 133);
const BACKGROUND: Color32 = Color32::from_rgb(24, 40, 45);
const TARGET: Color32 = Color32::from_rgb(255, 202, 133);
const MUTED: Color32 = Color32::from_rgb(164, 188, 186);

#[derive(Clone, Copy, PartialEq)]
struct Viewport {
    screen: Rect,
    center: Point,
    scale: f32,
}

impl Viewport {
    fn project(self, point: Point) -> Pos2 {
        self.screen.center()
            + egui::vec2(point.x - self.center.x, point.y - self.center.y) * self.scale
    }

    fn unproject(self, position: Pos2) -> Point {
        let delta = (position - self.screen.center()) / self.scale;
        Point {
            x: self.center.x + delta.x,
            y: self.center.y + delta.y,
        }
    }

    fn bounds(self) -> Bounds {
        Bounds {
            min: self.unproject(self.screen.min),
            max: self.unproject(self.screen.max),
        }
    }
}

pub enum MapClick {
    Agent(Option<usize>),
    Road(usize),
    MissingRoad,
}

pub struct MapView {
    center: Point,
    zoom: f32,
    roads: Option<(Viewport, Arc<Mesh>)>,
}

impl MapView {
    pub fn new(bounds: Bounds) -> Self {
        Self {
            center: bounds.center(),
            zoom: 1.0,
            roads: None,
        }
    }

    pub fn focus(&mut self, point: Point) {
        self.center = point;
    }

    pub fn fit(&mut self, bounds: Bounds) {
        self.center = bounds.center();
        self.zoom = 1.0;
    }

    pub fn zoom(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(0.72, 30.0);
    }

    fn viewport(&self, screen: Rect, bounds: Bounds) -> Viewport {
        let base_scale = (0.89 * screen.width() / (bounds.max.x - bounds.min.x))
            .min(0.86 * screen.height() / (bounds.max.y - bounds.min.y));
        Viewport {
            screen,
            center: self.center,
            scale: (base_scale * self.zoom).max(0.0001),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        simulation: &Simulation,
        selected: Option<usize>,
        closing: bool,
        hint: &str,
    ) -> Option<MapClick> {
        let (screen, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        let bounds = simulation.network.bounds;
        let mut view = self.viewport(screen, bounds);

        if response.dragged_by(egui::PointerButton::Primary) {
            let delta = response.drag_delta() / view.scale;
            self.center.x -= delta.x;
            self.center.y -= delta.y;
            view = self.viewport(screen, bounds);
        }
        if let Some(position) = response.hover_pos() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let anchor = view.unproject(position);
                self.zoom((0.003 * scroll).exp());
                let new_position = self.viewport(screen, bounds).unproject(position);
                self.center.x += anchor.x - new_position.x;
                self.center.y += anchor.y - new_position.y;
                view = self.viewport(screen, bounds);
            }
        }
        let cursor = if response.dragged() {
            egui::CursorIcon::Grabbing
        } else if closing {
            egui::CursorIcon::Crosshair
        } else {
            egui::CursorIcon::Grab
        };
        let response = response.on_hover_cursor(cursor);
        let painter = ui.painter_at(screen);
        painter.rect_filled(screen, 8.0, BACKGROUND);
        let meters = draw_grid(&painter, view);

        if self
            .roads
            .as_ref()
            .is_none_or(|(cached, _)| *cached != view)
        {
            self.roads = Some((view, Arc::new(road_mesh(simulation, view))));
        }
        if let Some((_, mesh)) = &self.roads {
            painter.add(Shape::Mesh(mesh.clone()));
        }
        for &index in simulation.closed_roads() {
            let road = &simulation.network.roads[index];
            if road.bounds.intersects(view.bounds()) {
                let points = road
                    .points
                    .iter()
                    .map(|&point| view.project(point))
                    .collect::<Vec<_>>();
                painter.add(Shape::line(
                    points.clone(),
                    Stroke::new(6.0_f32, BACKGROUND),
                ));
                painter.add(Shape::line(points, Stroke::new(3.0_f32, CLOSED)));
            }
        }
        if let Some(index) = selected {
            for portion in simulation.route(index).portions {
                let points =
                    simulation.network.roads[portion.road].portion(portion.from, portion.to);
                painter.add(Shape::line(
                    points
                        .into_iter()
                        .map(|point| view.project(point))
                        .collect(),
                    Stroke::new(2.7_f32, Color32::from_rgb(176, 252, 227)),
                ));
            }
        }
        for index in 0..AGENT_COUNT {
            let point = view.project(simulation.position(index));
            if !screen.expand(6.0).contains(point) {
                continue;
            }
            let color = match simulation.status(index) {
                Status::Traveling => ACCENT,
                Status::Arrived => Color32::from_rgb(93, 140, 125),
                Status::Stranded => CLOSED,
            };
            painter.circle_filled(point, 2.5, color);
        }
        for (&node, label) in simulation.targets.iter().zip(TARGET_LABELS) {
            let point = view.project(simulation.network.nodes[node]);
            if !screen.expand(12.0).contains(point) {
                continue;
            }
            let shape = vec![
                point - egui::vec2(0.0, 12.0),
                point + egui::vec2(12.0, 0.0),
                point + egui::vec2(0.0, 12.0),
                point - egui::vec2(12.0, 0.0),
            ];
            painter.add(Shape::convex_polygon(
                shape,
                Color32::from_rgb(46, 40, 33),
                Stroke::new(1.5_f32, TARGET),
            ));
            painter.text(
                point,
                egui::Align2::CENTER_CENTER,
                label,
                FontId::proportional(13.0),
                TARGET,
            );
        }
        if let Some(index) = selected {
            painter.circle_stroke(
                view.project(simulation.position(index)),
                7.0,
                Stroke::new(1.9_f32, ACCENT),
            );
        }
        draw_map_labels(&painter, view, meters, hint);

        let click = response.interact_pointer_pos()?;
        if response.secondary_clicked() || (closing && response.clicked()) {
            let world = view.unproject(click);
            let radius = 13.0 / view.scale;
            let nearest = simulation
                .network
                .roads
                .iter()
                .enumerate()
                .filter(|(_, road)| road.bounds.contains_near(world, radius))
                .map(|(index, road)| (index, road.distance_squared_to(world)))
                .filter(|&(_, distance)| distance <= radius * radius)
                .min_by(|(_, a), (_, b)| a.total_cmp(b));
            return Some(nearest.map_or(MapClick::MissingRoad, |(index, _)| MapClick::Road(index)));
        }
        if response.clicked() {
            let nearest = (0..AGENT_COUNT)
                .map(|index| {
                    (
                        index,
                        view.project(simulation.position(index)).distance_sq(click),
                    )
                })
                .filter(|&(_, distance)| distance <= 14.0f32.powi(2))
                .min_by(|(_, a), (_, b)| a.total_cmp(b))
                .map(|(index, _)| index);
            return Some(MapClick::Agent(nearest));
        }
        None
    }
}

fn road_mesh(simulation: &Simulation, view: Viewport) -> Mesh {
    let mut mesh = Mesh::default();
    let bounds = view.bounds();
    for level in 0..=2 {
        for road in &simulation.network.roads {
            if road.level != level || !road.bounds.intersects(bounds) {
                continue;
            }
            let (mut color, width) = match level {
                2 => (Color32::from_rgb(143, 164, 163), 1.45),
                1 => (Color32::from_rgb(94, 124, 128), 1.15),
                _ => (Color32::from_rgb(62, 83, 86), 0.85),
            };
            if road.bridge {
                color = Color32::from_rgb(115, 156, 159);
            }
            for segment in road.points.windows(2) {
                let start = view.project(segment[0]);
                let end = view.project(segment[1]);
                let direction = end - start;
                if direction.length_sq() <= 0.0001 {
                    continue;
                }
                let offset = Vec2::new(-direction.y, direction.x).normalized() * (width * 0.5);
                let index = mesh.vertices.len() as u32;
                for point in [start + offset, start - offset, end - offset, end + offset] {
                    mesh.colored_vertex(point, color);
                }
                mesh.add_triangle(index, index + 1, index + 2);
                mesh.add_triangle(index, index + 2, index + 3);
            }
        }
    }
    mesh
}

fn draw_grid(painter: &egui::Painter, view: Viewport) -> f32 {
    let meters = [50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0]
        .into_iter()
        .rfind(|&meters| meters * view.scale <= 95.0)
        .unwrap_or(50.0);
    let step = meters * view.scale;
    let origin = view.project(Point::default()) - view.screen.min;
    let stroke = Stroke::new(0.75_f32, Color32::from_rgb(35, 55, 59));
    let mut x = view.screen.left() + origin.x.rem_euclid(step);
    while x < view.screen.right() {
        painter.line_segment(
            [
                egui::pos2(x, view.screen.top()),
                egui::pos2(x, view.screen.bottom()),
            ],
            stroke,
        );
        x += step;
    }
    let mut y = view.screen.top() + origin.y.rem_euclid(step);
    while y < view.screen.bottom() {
        painter.line_segment(
            [
                egui::pos2(view.screen.left(), y),
                egui::pos2(view.screen.right(), y),
            ],
            stroke,
        );
        y += step;
    }
    meters
}

fn draw_map_labels(painter: &egui::Painter, view: Viewport, meters: f32, hint: &str) {
    let text_position = view.screen.left_top() + egui::vec2(16.0, 14.0);
    let hint = painter.layout_no_wrap(hint.to_owned(), FontId::proportional(13.0), MUTED);
    painter.rect_filled(
        Rect::from_min_size(
            text_position - egui::vec2(6.0, 4.0),
            hint.size() + egui::vec2(12.0, 8.0),
        ),
        5.0,
        Color32::from_rgb(16, 29, 33),
    );
    painter.galley(text_position, hint, MUTED);
    painter.text(
        view.screen.right_top() + egui::vec2(-16.0, 12.0),
        egui::Align2::RIGHT_TOP,
        "Š",
        FontId::proportional(15.0),
        MUTED,
    );

    let origin = view.screen.left_bottom() + egui::vec2(15.0, -15.0);
    let end = origin + egui::vec2(meters * view.scale, 0.0);
    let stroke = Stroke::new(1.0_f32, MUTED);
    painter.line_segment([origin, end], stroke);
    for point in [origin, end] {
        painter.line_segment([point, point - egui::vec2(0.0, 5.0)], stroke);
    }
    let scale_label = if meters >= 1000.0 {
        format!("{} km", meters / 1000.0)
    } else {
        format!("{} m", meters)
    };
    painter.text(
        origin + (end - origin) * 0.5 - egui::vec2(0.0, 7.0),
        egui::Align2::CENTER_BOTTOM,
        scale_label,
        FontId::proportional(12.0),
        MUTED,
    );
}
