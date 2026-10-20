//! A2 switch: discover the network, isolate customers, and choose whole paths.
//! No world names, switch layouts, customer addresses, or burst times are built in.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use switch_program_sdk::*;

const HELLO: u8 = 89;
const REPORT: u8 = 90;
const CONTROL_IP: u32 = 0xe000_0005;
const TICK: u64 = 50_000_000;
const SECOND: u64 = 1_000_000_000;

// Tables: protocol, customer queue, destination, source path, path forwarding.
const PROTOCOL: u32 = 1;
const CUSTOMER: u32 = 2;
const DESTINATION: u32 = 3;
const SOURCE_PATH: u32 = 4;
const PATH: u32 = 5;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Link {
    to: u32,
    capacity: u64,
    level: u8,
    customers: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Route {
    source: u32,
    destination: u32,
    id: u32,
    nodes: Vec<u32>,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
struct Report {
    links: Vec<Link>,
    hosts: Vec<(u32, u16)>,
    routes: Vec<Route>,
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    origin: u32,
    sequence: u64,
    report: Report,
}

pub struct FairSwitch {
    id: u32,
    ports: Vec<u16>,
    capacities: BTreeMap<u16, u64>,
    customers: Vec<Customer>,
    neighbours: BTreeMap<u16, u32>,
    hosts: BTreeMap<u32, u16>,
    peers: BTreeMap<u32, u32>,
    reports: BTreeMap<u32, Report>,
    seen: BTreeMap<u32, u64>,
    sequence: u64,
    // Per-port drop counts, then the last two 500 ms samples.
    drops: BTreeMap<u16, Vec<u64>>,
    history: BTreeMap<u16, VecDeque<Vec<u64>>>,
    routes: BTreeMap<u32, Route>,
    pending: BTreeMap<u32, u64>,
    changed_at: BTreeMap<u32, u64>,
    generation: u32,
    installed: BTreeMap<(u32, u64), u64>,
    queue_shapes: BTreeMap<u16, Vec<(u32, u32)>>,
    retired: BTreeMap<u32, u64>,
    now: u64,
    ticks: u64,
}

fn entry(id: u64, key: u64, prefix_len: u8, action: TableAction) -> TableEntry {
    TableEntry {
        id: wire::EntryIdW(id),
        key,
        prefix_len,
        priority: 0,
        action,
    }
}

fn contains(customer: &Customer, ip: u32) -> bool {
    let mask = u32::MAX
        .checked_shl(32 - customer.prefix_len as u32)
        .unwrap_or(0);
    ip & mask == customer.prefix_addr & mask
}

impl FairSwitch {
    fn customer(&self, ip: u32) -> Option<usize> {
        self.customers.iter().position(|c| contains(c, ip))
    }

    fn owner(&self, ip: u32) -> Option<(u32, u16)> {
        self.reports.iter().find_map(|(&node, report)| {
            report
                .hosts
                .iter()
                .find(|(host, _)| *host == ip)
                .map(|(_, port)| (node, *port))
        })
    }

    fn next_port(&self, node: u32) -> Option<u16> {
        self.neighbours
            .iter()
            .find(|(_, peer)| **peer == node)
            .map(|(&port, _)| port)
    }

    fn inject(&self, port: u16, protocol: u8, payload: Vec<u8>) -> Action {
        actions::inject_packet(
            port,
            0xa9fe_0000 | (self.id & 0xffff),
            CONTROL_IP,
            protocol,
            1,
            payload,
        )
    }

    fn flood(&self, bytes: &[u8], skip: Option<u16>, out: &mut Vec<Action>) {
        for &port in self.neighbours.keys() {
            if Some(port) != skip {
                out.push(self.inject(port, REPORT, bytes.to_vec()));
            }
        }
    }

    fn my_report(&self) -> Report {
        let links = self
            .neighbours
            .iter()
            .map(|(&port, &to)| {
                let samples = self.history.get(&port);
                let drops = samples
                    .into_iter()
                    .flatten()
                    .map(|q| q.iter().skip(1).sum::<u64>())
                    .max()
                    .unwrap_or(0);
                let customers = samples
                    .into_iter()
                    .flatten()
                    .map(|q| q.iter().skip(1).filter(|&&d| d > 0).count())
                    .max()
                    .unwrap_or(0);
                Link {
                    to,
                    capacity: self.capacities[&port],
                    level: match drops {
                        0 => 0,
                        1..=4 => 1,
                        5..=24 => 2,
                        _ => 3,
                    },
                    customers: customers as u8,
                }
            })
            .collect();
        Report {
            links,
            hosts: self.hosts.iter().map(|(&ip, &port)| (ip, port)).collect(),
            routes: self.routes.values().cloned().collect(),
        }
    }

    fn advertise(&mut self, out: &mut Vec<Action>) {
        let report = self.my_report();
        self.reports.insert(self.id, report.clone());
        self.sequence += 1;
        let bytes = postcard::to_allocvec(&Envelope {
            origin: self.id,
            sequence: self.sequence,
            report,
        })
        .unwrap();
        self.flood(&bytes, None, out);
    }

    fn install_port(&mut self, table: u32, key: u64, port: u16, out: &mut Vec<Action>) {
        let previous = self.installed.insert((table, key), port as u64);
        if previous != Some(port as u64) {
            // The kit's install appends; it does not replace an existing ID.
            if previous.is_some() {
                out.push(actions::delete_entry(table, key));
            }
            out.push(actions::install_route(table, key, key, 32, port));
        }
    }

    // Breadth-first distances also bound the path enumeration below.
    fn distances(&self, destination: u32) -> BTreeMap<u32, usize> {
        let mut distance = BTreeMap::from([(destination, 0)]);
        let mut todo = VecDeque::from([destination]);
        while let Some(node) = todo.pop_front() {
            let d = distance[&node] + 1;
            for (&other, report) in &self.reports {
                if !distance.contains_key(&other) && report.links.iter().any(|l| l.to == node) {
                    distance.insert(other, d);
                    todo.push_back(other);
                }
            }
        }
        distance
    }

    fn paths(&self, destination: u32) -> Vec<Vec<u32>> {
        let distances = self.distances(destination);
        let Some(&shortest) = distances.get(&self.id) else {
            return vec![];
        };
        let mut paths = Vec::new();
        self.walk(
            destination,
            shortest + 2,
            &distances,
            &mut vec![self.id],
            &mut paths,
        );
        paths
    }

    fn walk(
        &self,
        destination: u32,
        limit: usize,
        distances: &BTreeMap<u32, usize>,
        path: &mut Vec<u32>,
        paths: &mut Vec<Vec<u32>>,
    ) {
        let node = *path.last().unwrap();
        if node == destination {
            paths.push(path.clone());
            return;
        }
        if paths.len() >= 128 {
            return;
        }
        if let Some(report) = self.reports.get(&node) {
            for link in &report.links {
                if path.contains(&link.to)
                    || path.len() + distances.get(&link.to).copied().unwrap_or(100) > limit
                {
                    continue;
                }
                path.push(link.to);
                self.walk(destination, limit, distances, path, paths);
                path.pop();
            }
        }
    }

    fn known_routes(&self) -> Vec<Route> {
        self.reports
            .values()
            .flat_map(|r| r.routes.iter().cloned())
            .collect()
    }

    // Estimate our max-min share on each link, capped by the contract.
    // Small customers take their demand; the remaining users share the rest.
    fn path_score(&self, source: u32, path: &[u32]) -> f64 {
        let demand = self
            .customer(source)
            .and_then(|i| self.customers[i].ask_bps)
            .unwrap_or(u64::MAX) as f64;
        let routes = self.known_routes();
        let mut score = demand;
        for edge in path.windows(2) {
            let Some(link) = self
                .reports
                .get(&edge[0])
                .and_then(|r| r.links.iter().find(|l| l.to == edge[1]))
            else {
                return 0.0;
            };
            let mut asks = vec![demand];
            for route in &routes {
                if route.source != source && route.nodes.windows(2).any(|e| e == edge) {
                    asks.push(
                        self.customer(route.source)
                            .and_then(|i| self.customers[i].ask_bps)
                            .unwrap_or(u64::MAX) as f64,
                    );
                }
            }
            asks.sort_by(f64::total_cmp);
            let mut remaining = link.capacity as f64;
            let mut fair = remaining;
            for (i, ask) in asks.iter().enumerate() {
                fair = remaining / (asks.len() - i) as f64;
                if *ask >= fair {
                    break;
                }
                remaining -= ask;
            }
            // Drop reports are a conservative penalty, not proof a link is dead.
            score = score.min(fair * (1.0 - 0.025 * link.level as f64));
        }
        score
    }

    fn choose_routes(&mut self, out: &mut Vec<Action>) {
        let peers: Vec<_> = self.peers.iter().map(|(&a, &b)| (a, b)).collect();
        for (source, destination) in peers {
            if self.pending.contains_key(&source) {
                continue;
            }
            if self
                .changed_at
                .get(&source)
                .is_some_and(|&t| self.now < t + 2 * SECOND)
            {
                continue;
            }
            let Some((target, _)) = self.owner(destination) else {
                continue;
            };
            let mut paths: Vec<_> = self
                .paths(target)
                .into_iter()
                .map(|path| (self.path_score(source, &path), path))
                .collect();
            paths.sort_by(|(a_score, a), (b_score, b)| {
                b_score
                    .total_cmp(a_score)
                    .then(a.len().cmp(&b.len()))
                    .then(a.cmp(b))
            });
            let Some((best_score, best)) = paths.first() else {
                continue;
            };
            if let Some(old) = self.routes.get(&source)
                && (old.nodes == *best || *best_score <= self.path_score(source, &old.nodes) * 1.25)
            {
                continue;
            }
            self.generation = self.generation.wrapping_add(1) & 0xffff;
            // The customer index is identical at every switch. Low 16 bits
            // are a version, so old packets keep their old forwarding path.
            let Some(index) = self.customer(source) else {
                continue;
            };
            let id = (((index + 1) as u32) << 16) | self.generation;
            self.routes.insert(
                source,
                Route {
                    source,
                    destination,
                    id,
                    nodes: best.clone(),
                },
            );
            self.pending.insert(source, self.now + 200_000_000);
            self.changed_at.insert(source, self.now);
            self.advertise(out);
        }
    }

    fn refresh_forwarding(&mut self, out: &mut Vec<Action>) {
        // ACKs use ordinary shortest paths. Numbered data uses versioned paths.
        let hosts: Vec<_> = self
            .reports
            .iter()
            .flat_map(|(&node, r)| r.hosts.iter().map(move |&(ip, port)| (node, ip, port)))
            .collect();
        for (node, ip, local_port) in hosts {
            let port = if node == self.id {
                Some(local_port)
            } else {
                let distances = self.distances(node);
                self.neighbours
                    .iter()
                    .filter_map(|(&port, peer)| distances.get(peer).map(|&d| (d, *peer, port)))
                    .min()
                    .map(|(_, _, port)| port)
            };
            if let Some(port) = port {
                self.install_port(DESTINATION, ip as u64, port, out);
            }
        }
        for route in self.known_routes() {
            if let Some(i) = route.nodes.iter().position(|&node| node == self.id) {
                let port = if i + 1 == route.nodes.len() {
                    self.hosts.get(&route.destination).copied()
                } else {
                    self.next_port(route.nodes[i + 1])
                };
                if let Some(port) = port {
                    self.install_port(PATH, route.id as u64, port, out);
                }
            }
        }
        // Leave old paths in place for two seconds while their last packets
        // drain. Then reclaim them, keeping the forwarding table bounded.
        let mut live: BTreeSet<u32> = self.known_routes().iter().map(|r| r.id).collect();
        for (&(table, _), &id) in &self.installed {
            if table == SOURCE_PATH {
                live.insert(id as u32);
            }
        }
        for &(table, id) in self.installed.keys() {
            if table == PATH {
                if live.contains(&(id as u32)) {
                    self.retired.remove(&(id as u32));
                } else {
                    self.retired
                        .entry(id as u32)
                        .or_insert(self.now + 2 * SECOND);
                }
            }
        }
    }

    fn configure_queues(&mut self, out: &mut Vec<Action>) {
        // Only paths using this switch need data-buffer reservations here.
        let mut used: BTreeMap<u16, Vec<usize>> = BTreeMap::new();
        for route in self.known_routes() {
            if let Some(i) = route.nodes.iter().position(|&node| node == self.id) {
                let port = if i + 1 == route.nodes.len() {
                    self.hosts.get(&route.destination).copied()
                } else {
                    self.next_port(route.nodes[i + 1])
                };
                if let (Some(port), Some(customer)) = (port, self.customer(route.source)) {
                    let list = used.entry(port).or_default();
                    if !list.contains(&customer) {
                        list.push(customer);
                    }
                }
            }
        }
        let count = used.values().map(Vec::len).sum::<usize>().max(1) as u32;
        let allowance = (30 / count).clamp(2, 12);
        for port in self.ports.clone() {
            let mut shapes = vec![(2, 1)];
            for (i, customer) in self.customers.iter().enumerate() {
                let active = used.get(&port).is_some_and(|v| v.contains(&i));
                let cap = if !active {
                    1
                } else {
                    match customer.cca {
                        Cca::Aimd => allowance,
                        Cca::RateMatch => allowance.min(4),
                    }
                };
                shapes.push((cap, 1));
            }
            if self.queue_shapes.get(&port) != Some(&shapes) {
                let specs = shapes
                    .iter()
                    .map(|&(cap, weight)| QueueSpec::new(cap + 1).weight(weight).drop_at(cap))
                    .collect();
                out.push(actions::set_queues(port, Discipline::Drr, specs));
                self.queue_shapes.insert(port, shapes);
            }
        }
    }
}

impl SwitchProgram for FairSwitch {
    fn init(id: u32, ports: Vec<u16>) -> (Self, ProgramSetup) {
        Self::init_with(InitInput {
            switch_id: id,
            port_capacity_bps: vec![10_000_000; ports.len()],
            local_ports: ports,
            customers: vec![],
        })
    }

    fn init_with(input: InitInput) -> (Self, ProgramSetup) {
        let mut setup = ProgramSetup::new();
        for (table, kind, count) in [
            (PROTOCOL, MatchKind::Exact, 4),
            (CUSTOMER, MatchKind::Lpm, 16),
            (DESTINATION, MatchKind::Exact, 256),
            (SOURCE_PATH, MatchKind::Lpm, 16),
            (PATH, MatchKind::Exact, 1024),
        ] {
            setup.declare_table(table, kind, count);
        }
        setup.set_program(text::parse_tiny_program(include_str!("pipeline.tiny")).unwrap());
        for protocol in [HELLO, REPORT] {
            setup.add_entry(
                PROTOCOL,
                entry(
                    protocol as u64,
                    protocol as u64,
                    8,
                    TableAction::Punt {
                        reason: PuntReason::Custom(protocol as u32),
                    },
                ),
            );
        }
        for (i, customer) in input.customers.iter().enumerate() {
            setup.add_entry(
                CUSTOMER,
                entry(
                    i as u64 + 1,
                    customer.prefix_addr as u64,
                    customer.prefix_len,
                    TableAction::SetQueue {
                        queue: wire::QueueIdW((i + 1) as u16),
                    },
                ),
            );
        }
        let me = Self {
            id: input.switch_id,
            capacities: input
                .local_ports
                .iter()
                .copied()
                .zip(input.port_capacity_bps)
                .collect(),
            ports: input.local_ports,
            customers: input.customers,
            neighbours: BTreeMap::new(),
            hosts: BTreeMap::new(),
            peers: BTreeMap::new(),
            reports: BTreeMap::new(),
            seen: BTreeMap::new(),
            sequence: 0,
            drops: BTreeMap::new(),
            history: BTreeMap::new(),
            routes: BTreeMap::new(),
            pending: BTreeMap::new(),
            changed_at: BTreeMap::new(),
            generation: 0,
            installed: BTreeMap::new(),
            queue_shapes: BTreeMap::new(),
            retired: BTreeMap::new(),
            now: 0,
            ticks: 0,
        };
        (me, setup)
    }

    fn on_punt(&mut self, event: PuntEvent) -> Vec<Action> {
        self.now = event.now_ns;
        let mut out = Vec::new();
        match event.ip_proto {
            HELLO if event.payload.len() == 4 => {
                let peer = u32::from_le_bytes(event.payload[..4].try_into().unwrap());
                if self.neighbours.insert(event.ingress_port, peer).is_none() {
                    out.push(actions::link_up(event.ingress_port));
                    self.advertise(&mut out);
                }
            }
            REPORT => {
                if let Ok(envelope) = postcard::from_bytes::<Envelope>(&event.payload)
                    && envelope.origin != self.id
                    && envelope.sequence > self.seen.get(&envelope.origin).copied().unwrap_or(0)
                {
                    self.seen.insert(envelope.origin, envelope.sequence);
                    self.reports.insert(envelope.origin, envelope.report);
                    self.flood(&event.payload, Some(event.ingress_port), &mut out);
                    if self.now >= 300_000_000 {
                        self.refresh_forwarding(&mut out);
                    }
                }
            }
            _ => {
                // Apps announce with a zero-byte packet to their own address.
                if event.packet_size == 0 && event.ip_src == event.ip_dst {
                    self.hosts.insert(event.ip_src, event.ingress_port);
                    out.push(actions::link_up(event.ingress_port));
                    self.advertise(&mut out);
                } else if self.hosts.get(&event.ip_src) == Some(&event.ingress_port)
                    && self.customer(event.ip_src).is_some()
                    && event.ip_src != event.ip_dst
                {
                    self.peers.insert(event.ip_src, event.ip_dst);
                }
            }
        }
        out
    }

    fn on_timer(&mut self, event: TimerEvent) -> Vec<Action> {
        self.now = event.now_ns;
        self.ticks += 1;
        let mut out = Vec::new();
        for drop in event.port_drops {
            let counts = self.drops.entry(drop.port).or_default();
            counts.resize(counts.len().max(drop.queues.len()), 0);
            for (i, n) in drop.queues.iter().enumerate() {
                counts[i] += *n as u64;
            }
        }
        if self.ticks <= 6 || self.ticks.is_multiple_of(10) {
            for &port in &self.ports {
                out.push(self.inject(port, HELLO, self.id.to_le_bytes().to_vec()));
            }
        }
        if self.ticks.is_multiple_of(10) {
            for &port in &self.ports {
                let history = self.history.entry(port).or_default();
                history.push_back(self.drops.remove(&port).unwrap_or_default());
                if history.len() > 2 {
                    history.pop_front();
                }
            }
            self.advertise(&mut out);
        }
        if self.now >= 300_000_000 {
            self.refresh_forwarding(&mut out);
            // Stagger decisions by switch identity, so one report can arrive
            // before a neighbour decides to move its customer too.
            if self.ticks % 10 == self.id as u64 % 10 {
                self.choose_routes(&mut out);
            }
            self.configure_queues(&mut out);
        } else if self.ticks == 1 {
            self.configure_queues(&mut out);
        }
        let ready: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, at)| self.now >= **at)
            .map(|(&ip, _)| ip)
            .collect();
        for ip in ready {
            let route = &self.routes[&ip];
            if self
                .installed
                .insert((SOURCE_PATH, ip as u64), route.id as u64)
                .is_some()
            {
                out.push(actions::delete_entry(SOURCE_PATH, ip as u64));
            }
            out.push(actions::install_entry(
                SOURCE_PATH,
                entry(
                    ip as u64,
                    ip as u64,
                    32,
                    TableAction::SetMeta {
                        key: wire::MetaKeyW(3),
                        value: route.id as u64,
                    },
                ),
            ));
            self.pending.remove(&ip);
        }
        let expired: Vec<_> = self
            .retired
            .iter()
            .filter(|(_, at)| self.now >= **at)
            .map(|(&id, _)| id)
            .collect();
        for id in expired {
            out.push(actions::delete_entry(PATH, id as u64));
            self.installed.remove(&(PATH, id as u64));
            self.retired.remove(&id);
        }
        out.push(actions::schedule_timer(TICK));
        out
    }
}

switch_program!(FairSwitch);

#[cfg(test)]
mod tests;
