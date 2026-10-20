use super::*;

fn network() -> FairSwitch {
    let (mut switch, _) = FairSwitch::init_with(InitInput {
        switch_id: 0,
        local_ports: vec![1, 2, 100],
        port_capacity_bps: vec![2_000_000, 10_000_000, 10_000_000],
        customers: vec![
            Customer {
                prefix_addr: 0x0a000100,
                prefix_len: 24,
                cca: Cca::Aimd,
                ask_bps: None,
            },
            Customer {
                prefix_addr: 0x0a000200,
                prefix_len: 24,
                cca: Cca::RateMatch,
                ask_bps: Some(1_000_000),
            },
        ],
    });
    for (a, b, capacity) in [
        (0, 3, 2_000_000),
        (0, 1, 10_000_000),
        (1, 2, 10_000_000),
        (2, 3, 10_000_000),
    ] {
        for (from, to) in [(a, b), (b, a)] {
            switch.reports.entry(from).or_default().links.push(Link {
                to,
                capacity,
                level: 0,
                customers: 0,
            });
        }
    }
    switch.neighbours = BTreeMap::from([(1, 3), (2, 1)]);
    switch.hosts.insert(0x0a000101, 100);
    switch
        .reports
        .get_mut(&0)
        .unwrap()
        .hosts
        .push((0x0a000101, 100));
    switch
        .reports
        .get_mut(&3)
        .unwrap()
        .hosts
        .push((0x0a000301, 100));
    switch.peers.insert(0x0a000101, 0x0a000301);
    switch
}

#[test]
fn prefix_matching_handles_zero_and_host_prefixes() {
    let mut c = network().customers[0];
    assert!(contains(&c, 0x0a0001ff));
    assert!(!contains(&c, 0x0a000201));
    c.prefix_len = 0;
    assert!(contains(&c, 0x12345678));
    c.prefix_len = 32;
    assert!(contains(&c, c.prefix_addr));
    assert!(!contains(&c, c.prefix_addr + 1));
}

#[test]
fn paths_are_simple_and_stay_within_the_detour_limit() {
    let switch = network();
    let paths = switch.paths(3);
    assert!(paths.contains(&vec![0, 3]));
    assert!(paths.contains(&vec![0, 1, 2, 3]));
    for path in paths {
        assert_eq!(path.iter().collect::<BTreeSet<_>>().len(), path.len());
        assert!(path.len() - 1 <= switch.distances(3)[&0] + 2);
    }
}

#[test]
fn small_customer_takes_its_demand_before_bulk_shares_the_rest() {
    let mut switch = network();
    switch.reports.get_mut(&0).unwrap().routes.push(Route {
        source: 0x0a000201,
        destination: 0x0a000301,
        id: 1,
        nodes: vec![0, 3],
    });
    assert_eq!(switch.path_score(0x0a000101, &[0, 3]), 1_000_000.0);
    assert_eq!(switch.path_score(0x0a000201, &[0, 3]), 1_000_000.0);
}

#[test]
fn chooses_capacity_and_waits_before_activating_the_new_path() {
    let mut switch = network();
    let mut out = vec![];
    switch.choose_routes(&mut out);
    assert_eq!(switch.routes[&0x0a000101].nodes, vec![0, 1, 2, 3]);
    let source_install = |a: &Action| {
        matches!(a,
        Action::InstallTableEntry { table_id, .. } if table_id.0 == SOURCE_PATH)
    };
    assert!(!out.iter().any(source_install));
    let before = switch.on_timer(TimerEvent {
        now_ns: 199_000_000,
        port_drops: vec![],
    });
    assert!(!before.iter().any(source_install));
    let after = switch.on_timer(TimerEvent {
        now_ns: 200_000_000,
        port_drops: vec![],
    });
    assert!(after.iter().any(source_install));
}

#[test]
fn obsolete_path_is_kept_for_drain_time_then_removed() {
    let mut switch = network();
    switch.installed.insert((PATH, 123), 1);
    switch.now = SECOND;
    switch.refresh_forwarding(&mut vec![]);
    assert_eq!(switch.retired[&123], 3 * SECOND);
    switch.on_timer(TimerEvent {
        now_ns: 2 * SECOND,
        port_drops: vec![],
    });
    assert!(switch.installed.contains_key(&(PATH, 123)));
    let actions = switch.on_timer(TimerEvent {
        now_ns: 3 * SECOND,
        port_drops: vec![],
    });
    assert!(!switch.installed.contains_key(&(PATH, 123)));
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, Action::DeleteTableEntry {
        table_id, entry_id
    } if table_id.0 == PATH && entry_id.0 == 123))
    );
}

#[test]
fn initial_setup_fits_the_host_output_limit() {
    let (_, setup) = FairSwitch::init_with(InitInput {
        switch_id: 42,
        local_ports: vec![0, 1, 2, 3, 100],
        port_capacity_bps: vec![10_000_000; 5],
        customers: (0..15)
            .map(|i| Customer {
                prefix_addr: 0x0a000000 + i * 256,
                prefix_len: 24,
                cca: Cca::Aimd,
                ask_bps: Some(5_000_000),
            })
            .collect(),
    });
    let setup = setup.into_wire();
    assert!(setup.program.stages.len() <= 8);
    assert!(postcard::to_allocvec(&setup).unwrap().len() <= 4096);
}

#[test]
fn route_updates_delete_the_old_entry_before_installing() {
    let mut switch = network();
    let mut out = vec![];
    switch.install_port(DESTINATION, 42, 1, &mut out);
    out.clear();
    switch.install_port(DESTINATION, 42, 2, &mut out);
    assert!(
        matches!(&out[0], Action::DeleteTableEntry { table_id, entry_id }
        if table_id.0 == DESTINATION && entry_id.0 == 42)
    );
    assert!(matches!(&out[1], Action::InstallTableEntry { table_id, .. }
        if table_id.0 == DESTINATION));

    let source = 0x0a000101;
    switch.installed.insert((SOURCE_PATH, source as u64), 123);
    switch.routes.insert(
        source,
        Route {
            source,
            destination: 0x0a000301,
            id: 124,
            nodes: vec![0, 3],
        },
    );
    switch.pending.insert(source, 0);
    let out = switch.on_timer(TimerEvent {
        now_ns: 0,
        port_drops: vec![],
    });
    let deletes = out
        .iter()
        .position(|a| {
            matches!(a, Action::DeleteTableEntry {
        table_id, entry_id
    } if table_id.0 == SOURCE_PATH && entry_id.0 == source as u64)
        })
        .unwrap();
    let installs = out
        .iter()
        .position(|a| {
            matches!(a, Action::InstallTableEntry {
        table_id, ..
    } if table_id.0 == SOURCE_PATH)
        })
        .unwrap();
    assert!(deletes < installs);
}
