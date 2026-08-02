use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct LinkInfo {
    pub path: PathInfo,
    pub quality: LinkQuality,
}

#[derive(Clone, Debug)]
pub struct PathInfo {
    pub real_distance_m: f32,
    pub has_los: bool,
}

#[derive(Clone, Debug)]
pub struct LinkQuality {}

pub fn debug_print_connectivity(graph: &HashMap<u32, HashMap<u32, LinkInfo>>) {
    println!("--- Connectivity Graph Debug ---");

    if graph.is_empty() {
        println!("Graph is empty.");
        return;
    }

    for (node_id, neighbors) in graph {
        println!("Node ID: {} is connected to:", node_id);

        if neighbors.is_empty() {
            println!("  (no connections)");
            continue;
        }

        for (neighbor_id, link_info) in neighbors {
            println!(
                "  -> Node {}: Distance={:.2}m, LOS={}, Quality={:?}",
                neighbor_id,
                link_info.path.real_distance_m,
                link_info.path.has_los,
                link_info.quality
            );
        }
    }
    println!("---------------------------------");
}
