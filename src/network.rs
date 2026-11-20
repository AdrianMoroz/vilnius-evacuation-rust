use serde::Deserialize;

pub const UNREACHABLE: u64 = u64::MAX;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn distance_squared(self, other: Self) -> f32 {
        (self.x - other.x).powi(2) + (self.y - other.y).powi(2)
    }

    pub fn interpolate(self, other: Self, fraction: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * fraction,
            y: self.y + (other.y - self.y) * fraction,
        }
    }
}

impl From<[f32; 2]> for Point {
    fn from([x, y]: [f32; 2]) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}

impl Bounds {
    fn enclosing(points: &[Point]) -> Self {
        let mut bounds = Self {
            min: points[0],
            max: points[0],
        };
        for point in points {
            bounds.min.x = bounds.min.x.min(point.x);
            bounds.min.y = bounds.min.y.min(point.y);
            bounds.max.x = bounds.max.x.max(point.x);
            bounds.max.y = bounds.max.y.max(point.y);
        }
        bounds
    }

    pub fn center(self) -> Point {
        self.min.interpolate(self.max, 0.5)
    }

    pub fn contains_near(self, point: Point, margin: f32) -> bool {
        (self.min.x - margin..=self.max.x + margin).contains(&point.x)
            && (self.min.y - margin..=self.max.y + margin).contains(&point.y)
    }

    pub fn intersects(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }
}

#[derive(Debug)]
pub struct Road {
    pub first: usize,
    pub level: u8,
    pub bridge: bool,
    pub name: Option<usize>,
    pub points: Vec<Point>,
    pub bounds: Bounds,
    cumulative: Vec<f32>,
}

impl Road {
    pub fn length(&self) -> f32 {
        self.cumulative[self.cumulative.len() - 1]
    }

    pub fn at(&self, fraction: f32) -> Point {
        let distance = self.length() * fraction.clamp(0.0, 1.0);
        let end = self
            .cumulative
            .partition_point(|&offset| offset <= distance);
        if end >= self.points.len() {
            return self.points[self.points.len() - 1];
        }
        let start = end - 1;
        let span = self.cumulative[end] - self.cumulative[start];
        self.points[start].interpolate(self.points[end], (distance - self.cumulative[start]) / span)
    }

    pub fn portion(&self, from: f32, to: f32) -> Vec<Point> {
        let low = self.length() * from.min(to);
        let high = self.length() * from.max(to);
        let start = self.cumulative.partition_point(|&offset| offset <= low);
        let end = self
            .cumulative
            .partition_point(|&offset| offset < high)
            .max(start);
        let mut points = Vec::with_capacity(end - start + 2);
        points.push(self.at(from));
        let interior = &self.points[start..end];
        if from <= to {
            points.extend_from_slice(interior);
        } else {
            points.extend(interior.iter().rev().copied());
        }
        points.push(self.at(to));
        points
    }

    pub fn distance_squared_to(&self, point: Point) -> f32 {
        self.points
            .windows(2)
            .map(|segment| distance_squared_to_segment(point, segment[0], segment[1]))
            .fold(f32::INFINITY, f32::min)
    }
}

fn distance_squared_to_segment(point: Point, start: Point, end: Point) -> f32 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0.0 {
        return point.distance_squared(start);
    }
    let offset = ((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared;
    point.distance_squared(start.interpolate(end, offset.clamp(0.0, 1.0)))
}

#[derive(Clone, Copy, Debug)]
pub struct DirectedEdge {
    pub from: usize,
    pub to: usize,
    pub road: usize,
    pub length_cm: u64,
}

impl DirectedEdge {
    pub fn meters(self) -> f64 {
        self.length_cm as f64 / 100.0
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoadData {
    source_date: String,
    nodes: Vec<[f32; 2]>,
    roads: Vec<(usize, usize, u8, u8, i32, Vec<f32>)>,
    arcs: Vec<(usize, usize, usize, f64)>,
    names: Vec<String>,
}

#[derive(Debug)]
pub struct Network {
    pub source_date: String,
    pub nodes: Vec<Point>,
    pub roads: Vec<Road>,
    pub edges: Vec<DirectedEdge>,
    pub incoming: Vec<Vec<usize>>,
    pub bounds: Bounds,
    names: Vec<String>,
}

impl Network {
    pub fn load() -> serde_json::Result<Self> {
        let data: RoadData = serde_json::from_str(include_str!("../assets/roads.json"))?;
        let nodes: Vec<Point> = data.nodes.into_iter().map(Point::from).collect();
        let bounds = Bounds::enclosing(&nodes);
        let roads = data
            .roads
            .into_iter()
            .map(|(first, last, level, bridge, name, shape)| {
                let mut points = Vec::with_capacity(shape.len() / 2 + 2);
                points.push(nodes[first]);
                points.extend(
                    shape
                        .chunks_exact(2)
                        .map(|pair| Point::from([pair[0], pair[1]])),
                );
                points.push(nodes[last]);
                let bounds = Bounds::enclosing(&points);
                let mut cumulative = vec![0.0];
                for segment in points.windows(2) {
                    let length = segment[0].distance_squared(segment[1]).sqrt();
                    cumulative.push(cumulative[cumulative.len() - 1] + length);
                }
                Road {
                    first,
                    level,
                    bridge: bridge != 0,
                    name: usize::try_from(name).ok(),
                    points,
                    bounds,
                    cumulative,
                }
            })
            .collect();
        let mut incoming = vec![Vec::new(); nodes.len()];
        let edges = data
            .arcs
            .into_iter()
            .enumerate()
            .map(|(index, (from, to, road, length))| {
                incoming[to].push(index);
                DirectedEdge {
                    from,
                    to,
                    road,
                    length_cm: ((length * 100.0).round() as u64).max(1),
                }
            })
            .collect();
        Ok(Self {
            source_date: data.source_date,
            nodes,
            bounds,
            roads,
            edges,
            incoming,
            names: data.names,
        })
    }

    pub fn road_name(&self, road: usize) -> String {
        self.roads[road]
            .name
            .and_then(|name| self.names.get(name))
            .cloned()
            .unwrap_or_else(|| format!("Atkarpa Nr. {}", road + 1))
    }
}
