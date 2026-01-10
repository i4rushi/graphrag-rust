use std::collections::HashMap;
//use rand::Rng;

use crate::graph_export::GraphData;

pub struct LouvainDetector {
    graph: GraphData,
}

impl LouvainDetector {
    pub fn new(graph: GraphData) -> Self {
        Self { graph }
    }

    /// Run Louvain community detection
    /// Returns: entity_id -> community_id
    pub fn detect_communities(&self) -> HashMap<String, usize> {
        let n = self.graph.entities.len();
        
        if n == 0 {
            return HashMap::new();
        }

        // Initialize: each node in its own community
        let mut communities: Vec<usize> = (0..n).collect();
        
        // Build adjacency list with weights
        let mut adj_list: Vec<HashMap<usize, f64>> = vec![HashMap::new(); n];
        let mut total_edges = 0.0;
        
        for &(source, target) in &self.graph.edges {
            *adj_list[source].entry(target).or_insert(0.0) += 1.0;
            *adj_list[target].entry(source).or_insert(0.0) += 1.0;
            total_edges += 2.0; // Undirected
        }

        // Calculate node degrees
        let mut degrees: Vec<f64> = vec![0.0; n];
        for (node, neighbors) in adj_list.iter().enumerate() {
            degrees[node] = neighbors.values().sum();
        }

        let m = total_edges / 2.0; // Total weight of edges

        // Louvain iteration
        let mut improved = true;
        let mut iteration = 0;
        const MAX_ITERATIONS: usize = 10;

        while improved && iteration < MAX_ITERATIONS {
            improved = false;
            iteration += 1;

            // Try moving each node to neighboring community
            for node in 0..n {
                let current_comm = communities[node];
                let mut best_comm = current_comm;
                let mut best_gain = 0.0;

                // Get neighboring communities
                let mut neighbor_comms = HashMap::new();
                for (&neighbor, &weight) in &adj_list[node] {
                    let comm = communities[neighbor];
                    *neighbor_comms.entry(comm).or_insert(0.0) += weight;
                }

                // Try each neighboring community
                for (&comm, &_weight_to_comm) in &neighbor_comms {
                    if comm == current_comm {
                        continue;
                    }

                    let gain = self.modularity_gain(
                        node,
                        current_comm,
                        comm,
                        &communities,
                        &degrees,
                        &neighbor_comms,
                        m,
                    );

                    if gain > best_gain {
                        best_gain = gain;
                        best_comm = comm;
                    }
                }

                // Move to best community if improvement found
                if best_comm != current_comm && best_gain > 0.0 {
                    communities[node] = best_comm;
                    improved = true;
                }
            }
        }

        // Renumber communities to be contiguous (0, 1, 2, ...)
        let unique_comms: std::collections::HashSet<_> = communities.iter().cloned().collect();
        let mut comm_mapping: HashMap<usize, usize> = HashMap::new();
        for (new_id, &old_id) in unique_comms.iter().enumerate() {
            comm_mapping.insert(old_id, new_id);
        }

        // Build result map
        let mut result = HashMap::new();
        for (idx, entity_id) in self.graph.entities.iter().enumerate() {
            let old_comm = communities[idx];
            let new_comm = comm_mapping[&old_comm];
            result.insert(entity_id.clone(), new_comm);
        }

        println!("Detected {} communities in {} iterations", unique_comms.len(), iteration);
        result
    }

    /// Calculate modularity gain from moving node to a new community
    fn modularity_gain(
        &self,
        node: usize,
        from_comm: usize,
        to_comm: usize,
        communities: &[usize],
        degrees: &[f64],
        neighbor_comms: &HashMap<usize, f64>,
        m: f64,
    ) -> f64 {
        let k_i = degrees[node];
        let k_i_in_to = neighbor_comms.get(&to_comm).copied().unwrap_or(0.0);
        let k_i_in_from = neighbor_comms.get(&from_comm).copied().unwrap_or(0.0);

        // Sum of degrees in communities
        let sigma_to: f64 = communities.iter()
            .enumerate()
            .filter(|&(_, &c)| c == to_comm)
            .map(|(i, _)| degrees[i])
            .sum();

        let sigma_from: f64 = communities.iter()
            .enumerate()
            .filter(|&(_, &c)| c == from_comm)
            .map(|(i, _)| degrees[i])
            .sum();

        let delta_q = 
            (k_i_in_to - k_i_in_from) / (2.0 * m) 
            - (k_i * (sigma_to - sigma_from + k_i)) / (2.0 * m * m);

        delta_q
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_graph() {
        let mut graph = GraphData::new();
        
        // Create a simple graph with 2 communities
        let a = graph.add_entity("A".to_string());
        let b = graph.add_entity("B".to_string());
        let c = graph.add_entity("C".to_string());
        let d = graph.add_entity("D".to_string());

        // Community 1: A-B
        graph.add_edge(a, b);
        graph.add_edge(b, a);
        
        // Community 2: C-D
        graph.add_edge(c, d);
        graph.add_edge(d, c);
        
        // Weak bridge
        graph.add_edge(b, c);

        let detector = LouvainDetector::new(graph);
        let communities = detector.detect_communities();

        println!("Communities: {:?}", communities);
        assert!(communities.len() > 0);
    }
}