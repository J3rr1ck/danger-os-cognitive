use danger_kernel::{IntentGraph, NodeId};
use danger_tools::ToolRegistry;
use petgraph::graph::DiGraph;
use petgraph::algo::toposort;
use anyhow::{Result, anyhow};
use std::collections::HashMap;

pub struct EventLog {
    traces: Vec<ExecutionTrace>,
}

impl EventLog {
    pub fn new() -> Self {
        Self { traces: Vec::new() }
    }

    pub fn push(&mut self, trace: ExecutionTrace) {
        self.traces.push(trace);
    }

    pub fn get_traces(&self) -> &[ExecutionTrace] {
        &self.traces
    }
}

pub struct ExecutionEngine {
    registry: ToolRegistry,
    log: std::sync::Arc<tokio::sync::Mutex<EventLog>>,
}

impl ExecutionEngine {
    pub fn new(registry: ToolRegistry) -> Self {
        Self { 
            registry,
            log: std::sync::Arc::new(tokio::sync::Mutex::new(EventLog::new())),
        }
    }

    pub async fn get_log(&self) -> std::sync::MutexGuard<'_, EventLog> {
        // This is a bit simplified for the prototype
        // In a real system, we'd have more robust log management
        unimplemented!("Use separate log access for real systems")
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

            // CAPABILITY CHECK
            if tool.capability() != node.capability {
                return Err(anyhow!(
                    "Capability mismatch for node {}: tool provides '{}' but node requested '{}'",
                    node_id, tool.capability(), node.capability
                ));
            }

            let start_time = std::time::Instant::now();
            let result = tool.execute(serde_json::to_value(&node.inputs)?).await;
            let duration = start_time.elapsed();

            let trace = match result {
                Ok(output) => {
                    ExecutionTrace {
                        node_id: node.id.clone(),
                        tool_name: node.tool_name.clone(),
                        status: "Success".to_string(),
                        output: Some(output),
                        error: None,
                        duration_ms: duration.as_millis(),
                    }
                }
                Err(e) => {
                    ExecutionTrace {
                        node_id: node.id.clone(),
                        tool_name: node.tool_name.clone(),
                        status: "Failed".to_string(),
                        output: None,
                        error: Some(e.to_string()),
                        duration_ms: duration.as_millis(),
                    }
                }
            };

            self.log.lock().await.push(trace.clone());
            traces.push(trace);

            if let Some(err) = &traces.last().unwrap().error {
                return Err(anyhow!("Execution failed at node {}: {}", node.tool_name, err));
            }
        }

        Ok(traces)
    }
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct ExecutionTrace {
    pub node_id: NodeId,
    pub tool_name: String,
    pub status: String,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u128,
}

