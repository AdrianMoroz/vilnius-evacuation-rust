use crate::network::{Network, Point, UNREACHABLE};
use std::{
    cmp::Reverse,
    collections::{BTreeSet, BinaryHeap},
};

pub const AGENT_COUNT: usize = 500;
pub const TARGET_LABELS: [char; 5] = ['A', 'B', 'C', 'D', 'E'];
const TARGET_ANCHORS: [(f64, f64); 5] = [
    (0.25, 0.33),
    (0.66, 0.24),
    (0.77, 0.59),
    (0.51, 0.75),
    (0.26, 0.63),
];
const SEED: u32 = 4509;
const METERS_PER_SECOND: f64 = 65.0;

struct Routes {
    distance_cm: Vec<u64>,
    next_edge: Vec<Option<usize>>,
    destinations: Vec<Option<usize>>,
}

fn routes_to_targets(network: &Network, targets: &[usize], closed: &BTreeSet<usize>) -> Routes {
    let mut routes = Routes {
        distance_cm: vec![UNREACHABLE; network.nodes.len()],
        next_edge: vec![None; network.nodes.len()],
        destinations: vec![None; network.nodes.len()],
    };
    let mut frontier = BinaryHeap::new();
    for (index, &target) in targets.iter().enumerate() {
        routes.distance_cm[target] = 0;
        routes.destinations[target] = Some(index);
        frontier.push(Reverse((0, target)));
    }
    while let Some(Reverse((distance, node))) = frontier.pop() {
        if distance != routes.distance_cm[node] {
            continue;
        }
        for &index in &network.incoming[node] {
            let edge = network.edges[index];
            let candidate = distance + edge.length_cm;
            if closed.contains(&edge.road) || candidate >= routes.distance_cm[edge.from] {
                continue;
            }
            routes.distance_cm[edge.from] = candidate;
            routes.next_edge[edge.from] = Some(index);
            routes.destinations[edge.from] = routes.destinations[node];
            frontier.push(Reverse((candidate, edge.from)));
        }
    }
    routes
}

fn choose_targets(network: &Network) -> Vec<usize> {
    let mut targets = Vec::with_capacity(TARGET_ANCHORS.len());
    for (x_fraction, y_fraction) in TARGET_ANCHORS {
        let bounds = network.bounds;
        let x = f64::from(bounds.min.x) + f64::from(bounds.max.x - bounds.min.x) * x_fraction;
        let y = f64::from(bounds.min.y) + f64::from(bounds.max.y - bounds.min.y) * y_fraction;
        let distance =
            |point: Point| (f64::from(point.x) - x).powi(2) + (f64::from(point.y) - y).powi(2);
        let (index, _) = network
            .nodes
            .iter()
            .copied()
            .enumerate()
            .filter(|(index, _)| network.incoming[*index].len() >= 2 && !targets.contains(index))
            .min_by(|(_, a), (_, b)| distance(*a).total_cmp(&distance(*b)))
            .expect("the bundled road network has enough connected intersections");
        targets.push(index);
    }
    targets
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Traveling,
    Arrived,
    Stranded,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Counts {
    pub traveling: usize,
    pub arrived: usize,
    pub stranded: usize,
}

#[derive(Clone, Copy, Debug)]
struct Travel {
    edge: usize,
    progress: f64,
    returning: bool,
}

#[derive(Clone, Copy, Debug)]
struct Agent {
    node: usize,
    travel: Option<Travel>,
}

#[derive(Debug)]
pub struct RoadPortion {
    pub road: usize,
    pub from: f32,
    pub to: f32,
}

pub struct AgentRoute {
    pub portions: Vec<RoadPortion>,
    pub destination: Option<char>,
    pub remaining_meters: Option<f64>,
    pub returning: bool,
}

pub struct Simulation {
    pub network: Network,
    pub targets: Vec<usize>,
    pub elapsed: f64,
    closed: BTreeSet<usize>,
    start_nodes: Vec<usize>,
    agents: Vec<Agent>,
    routes: Routes,
}

impl Simulation {
    pub fn new(network: Network) -> Self {
        let targets = choose_targets(&network);
        let closed = BTreeSet::new();
        let routes = routes_to_targets(&network, &targets, &closed);
        let candidates: Vec<_> = routes
            .distance_cm
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(node, distance)| {
                (distance != UNREACHABLE && distance != 0).then_some(node)
            })
            .collect();
        let mut seed = SEED;
        let start_nodes = (0..AGENT_COUNT)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let index = (u64::from(seed) * candidates.len() as u64) >> 32;
                candidates[index as usize]
            })
            .collect();
        let mut result = Self {
            network,
            targets,
            closed,
            start_nodes,
            agents: Vec::new(),
            routes,
            elapsed: 0.0,
        };
        result.restart_agents();
        result
    }

    fn restart_agents(&mut self) {
        self.elapsed = 0.0;
        self.agents = self
            .start_nodes
            .iter()
            .map(|&node| Agent { node, travel: None })
            .collect();
    }

    pub fn reset(&mut self) {
        if !self.closed.is_empty() {
            self.closed.clear();
            self.routes = routes_to_targets(&self.network, &self.targets, &self.closed);
        }
        self.restart_agents();
    }

    pub fn closed_roads(&self) -> &BTreeSet<usize> {
        &self.closed
    }

    pub fn set_closed(&mut self, road: usize, closed: bool) {
        let changed = if closed {
            self.closed.insert(road)
        } else {
            self.closed.remove(&road)
        };
        if !changed {
            return;
        }
        if closed {
            for agent in &mut self.agents {
                if let Some(travel) = &mut agent.travel
                    && self.network.edges[travel.edge].road == road
                {
                    travel.returning = true;
                }
            }
        }
        self.routes = routes_to_targets(&self.network, &self.targets, &self.closed);
    }

    pub fn step(&mut self, seconds: f64) {
        self.elapsed += seconds;
        for agent in &mut self.agents {
            let mut remaining = seconds * METERS_PER_SECOND;
            while remaining > 0.0 {
                if agent.travel.is_none() {
                    let Some(edge) = self.routes.next_edge[agent.node] else {
                        break;
                    };
                    agent.travel = Some(Travel {
                        edge,
                        progress: 0.0,
                        returning: false,
                    });
                }
                let travel = agent.travel.as_mut().expect("an active edge was selected");
                let edge = self.network.edges[travel.edge];
                let distance = if travel.returning {
                    travel.progress
                } else {
                    edge.meters() - travel.progress
                };
                if remaining >= distance {
                    remaining -= distance;
                    agent.node = if travel.returning { edge.from } else { edge.to };
                    agent.travel = None;
                } else {
                    travel.progress += if travel.returning {
                        -remaining
                    } else {
                        remaining
                    };
                    break;
                }
            }
        }
    }

    pub fn position(&self, index: usize) -> Point {
        let agent = self.agents[index];
        let Some(travel) = agent.travel else {
            return self.network.nodes[agent.node];
        };
        let edge = self.network.edges[travel.edge];
        let road = &self.network.roads[edge.road];
        let fraction = (travel.progress / edge.meters()) as f32;
        road.at(if edge.from == road.first {
            fraction
        } else {
            1.0 - fraction
        })
    }

    pub fn status(&self, index: usize) -> Status {
        let agent = self.agents[index];
        if agent.travel.is_some() {
            return Status::Traveling;
        }
        match self.routes.distance_cm[agent.node] {
            0 => Status::Arrived,
            UNREACHABLE => Status::Stranded,
            _ => Status::Traveling,
        }
    }

    pub fn counts(&self) -> Counts {
        let mut counts = Counts::default();
        for index in 0..self.agents.len() {
            match self.status(index) {
                Status::Traveling => counts.traveling += 1,
                Status::Arrived => counts.arrived += 1,
                Status::Stranded => counts.stranded += 1,
            }
        }
        counts
    }

    pub fn route(&self, index: usize) -> AgentRoute {
        let agent = self.agents[index];
        let mut node = agent.node;
        let mut portions = Vec::new();
        let mut current_remaining = 0.0;
        let mut returning = false;
        if let Some(travel) = agent.travel {
            let edge = self.network.edges[travel.edge];
            let from = (travel.progress / edge.meters()) as f32;
            let to = if travel.returning { 0.0 } else { 1.0 };
            portions.push(self.portion(travel.edge, from, to));
            current_remaining = if travel.returning {
                travel.progress
            } else {
                edge.meters() - travel.progress
            };
            node = if travel.returning { edge.from } else { edge.to };
            returning = travel.returning;
        }
        let destination = self.routes.destinations[node].map(|target| TARGET_LABELS[target]);
        let distance = self.routes.distance_cm[node];
        let remaining_meters =
            (distance != UNREACHABLE).then_some(current_remaining + distance as f64 / 100.0);
        while let Some(edge) = self.routes.next_edge[node] {
            portions.push(self.portion(edge, 0.0, 1.0));
            node = self.network.edges[edge].to;
        }
        AgentRoute {
            portions,
            destination,
            remaining_meters,
            returning,
        }
    }

    fn portion(&self, index: usize, from: f32, to: f32) -> RoadPortion {
        let edge = self.network.edges[index];
        let forward = edge.from == self.network.roads[edge.road].first;
        RoadPortion {
            road: edge.road,
            from: if forward { from } else { 1.0 - from },
            to: if forward { to } else { 1.0 - to },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scenario() -> Simulation {
        Simulation::new(Network::load().expect("valid bundled graph"))
    }

    fn assert_distance(route: &AgentRoute, expected_meters: f64) {
        assert!((route.remaining_meters.unwrap() - expected_meters).abs() < 0.001);
    }

    #[test]
    fn starts_and_arrival_counts_match_the_browser_scenario() {
        let mut simulation = scenario();
        assert_eq!(simulation.network.nodes.len(), 7_856);
        assert_eq!(simulation.network.roads.len(), 10_316);
        assert_eq!(simulation.network.edges.len(), 18_422);
        assert_eq!(simulation.targets, [1372, 7205, 7672, 506, 4493]);
        assert_eq!(
            simulation.start_nodes[..8],
            [7727, 2054, 1326, 4514, 3775, 393, 6988, 4507]
        );
        let mut previous = 0.0;
        for (seconds, arrived) in [(1.0, 0), (10.0, 3), (30.0, 15), (120.0, 288)] {
            simulation.step(seconds - previous);
            previous = seconds;
            assert_eq!(
                simulation.counts(),
                Counts {
                    traveling: AGENT_COUNT - arrived,
                    arrived,
                    stranded: 0
                }
            );
        }
        simulation.step(400.0);
        assert_eq!(simulation.counts().arrived, AGENT_COUNT);
    }

    #[test]
    fn routes_around_a_closed_road_and_reopens_it() {
        let mut simulation = scenario();
        let index = 41;
        let route = simulation.route(index);
        assert_eq!(route.destination, Some('D'));
        assert_eq!(route.portions[0].road, 5836);
        assert_distance(&route, 3217.57);

        simulation.set_closed(5836, true);
        let detour = simulation.route(index);
        assert_eq!(detour.destination, Some('D'));
        assert_distance(&detour, 3287.39);
        assert!(detour.portions.iter().all(|leg| leg.road != 5836));
        assert!(
            simulation
                .routes
                .next_edge
                .iter()
                .flatten()
                .all(|&edge| simulation.network.edges[edge].road != 5836)
        );

        simulation.set_closed(5836, false);
        assert_distance(&simulation.route(index), 3217.57);
    }

    #[test]
    fn an_agent_on_a_closed_road_returns_and_reset_restores_the_scenario() {
        let mut simulation = scenario();
        let index = 41;
        let start = simulation.agents[index].node;
        let first = simulation.network.edges[simulation.routes.next_edge[start].unwrap()];
        let duration = first.meters() / (4.0 * METERS_PER_SECOND);
        let original_position = simulation.position(index);
        simulation.step(duration);
        assert_ne!(simulation.position(index), original_position);

        simulation.set_closed(first.road, true);
        assert!(simulation.route(index).returning);
        simulation.step(duration);
        assert_eq!(simulation.agents[index].node, start);
        assert_eq!(simulation.position(index), original_position);
        assert!(!simulation.route(index).returning);
        assert_distance(&simulation.route(index), 3287.39);

        simulation.reset();
        assert!(simulation.closed_roads().is_empty());
        assert_eq!(simulation.elapsed, 0.0);
        assert_eq!(simulation.position(index), original_position);
        assert_eq!(simulation.counts().traveling, AGENT_COUNT);
        assert_distance(&simulation.route(index), 3217.57);
    }
}
