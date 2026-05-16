use danger_kernel::{IntentGraph, NodeId};
use danger_tools::ToolRegistry;
use petgraph::graph::DiGraph;
use petgraph::algo::toposort;
use anyhow::{Result, anyhow};
use std::collections::HashMap;

pub struct ExecutionEngine {
    registry: ToolRegistry,
}

impl ExecutionEngine {
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }

    pub async fn execute(&self, graph: IntentGraph) -> Result<Vec<ExecutionTrace>> {
        let mut g = DiGraph::<NodeId, ()>::new();
        let mut node_map = HashMap::new();
        let mut id_to_node = HashMap::new();

        for node in &graph.nodes {
            let idx = g.add_node(node.id.clone());
            node_map.insert(node.id.clone(), idx);
            id_to_node.insert(node.id.clone(), node);
        }

        for (src, dst) in &graph.edges {
            let src_idx = node_map.get(src).ok_or_else(|| anyhow!("Source node missing"))?;
            let dst_idx = node_map.get(dst).ok_or_else(|| anyhow!("Target node missing"))?;
            g.add_edge(*src_idx, *dst_idx, ());
        }

        let sorted_indices = toposort(&g, None)
            .map_err(|_| anyhow!("Cycle detected in execution graph during sort"))?;

        let mut traces = Vec::new();

        for idx in sorted_indices {
            let node_id = g.node_weight(idx).unwrap();
            let node = id_to_node.get(node_id).unwrap();
            
            println!("[ENGINE] Executing node: {} ({})", node.tool_name, node_id);
            
            let tool = self.registry.get(&node.tool_name)
                .ok_or_else(|| anyhow!("Tool not found: {}", node.tool_name))?;

            let start_time = std::time::Instant::now();
            let result = tool.execute(serde_json::to_value(&node.inputs)?).await;
            let duration = start_time.elapsed();

            match result {
                Ok(output) => {
                    traces.push(ExecutionTrace {
                        node_id: node.id.clone(),
                        tool_name: node.tool_name.clone(),
                        status: "Success".to_string(),
                        output: Some(output),
                        error: None,
                        duration_ms: duration.as_millis(),
                    });
                }
                Err(e) => {
                    traces.push(ExecutionTrace {
                        node_id: node.id.clone(),
                        tool_name: node.tool_name.clone(),
                        status: "Failed".to_string(),
                        output: None,
                        error: Some(e.to_string()),
                        duration_ms: duration.as_millis(),
                    });
                    return Err(anyhow!("Execution failed at node {}: {}", node.tool_name, e));
                }
            }
        }

        Ok(traces)
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ExecutionTrace {
    pub node_id: NodeId,
    pub tool_name: String,
    pub status: String,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u128,
}

