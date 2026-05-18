#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::String;
use alloc::string::ToString;
use alloc::format;
use serde::{Serialize, Deserialize};
use alloc::collections::BTreeMap;

// --- Danger OS Substrate ---

pub struct DangerOS {
    pub engine: ExecutionEngine,
    pub gate: SymbolicGate,
    pub vector_store: VectorStore,
    pub identity: IdentityLayer,
}

impl DangerOS {
    pub fn new() -> Self {
        let registry = default_registry();
        let policy = Policy::default_user();
        Self {
            engine: ExecutionEngine::new(registry),
            gate: SymbolicGate::new(policy),
            vector_store: VectorStore::new(),
            identity: IdentityLayer::new(),
        }
    }
    pub fn process_intent(
        &mut self, 
        intent: &str, 
        config: &inference::Config, 
        weights: &inference::Weights
    ) -> anyhow::Result<Vec<ExecutionTrace>> {
        // 0. Semantic Recall (Lookup)
        let mut state = inference::RunState::new(config);
        // Simplified embedding: use the first character's token as a query
        inference::forward(intent.as_bytes().get(0).cloned().unwrap_or(0) as usize, 0, config, weights, &mut state);
        
        let mut traces = Vec::new();
        if let Some(recalled) = self.vector_store.search(&state.logits) {
            traces.push(ExecutionTrace {
                node_id: "semantic_recall".to_string(),
                tool_name: "memory_lookup".to_string(),
                status: "Success".to_string(),
                output: format!("Recalled: {}", recalled),
                error: None,
            });
        }

        // 1. Compilation
        let graph = compiler::SyscallBridge::compile_model_driven(intent, config, weights);

        // 2. Validation
        self.gate.validate(&graph)?;

        // 3. Execution
        let exec_traces = self.engine.execute(&graph)?;

        // 4. Memory Logging (Post-Execution)
        for trace in &exec_traces {
            if trace.status == "Success" {
                // Store in semantic memory if we have an embedding
                if let Some(node) = graph.nodes.iter().find(|n| n.id == trace.node_id) {
                    if let Some(logits) = &node.raw_logits {
                        self.vector_store.store(logits.clone(), format!("Executed {}: {}", trace.tool_name, trace.output));
                    }
                }
            }
        }

        traces.extend(exec_traces);
        Ok(traces)
    }
}

pub type NodeId = String;

#[cfg(target_os = "none")]
pub mod memory;
#[cfg(target_os = "none")]
pub mod allocator;

pub mod inference;

// --- Memory System ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExecutionTrace {
    pub node_id: NodeId,
    pub tool_name: String,
    pub status: String,
    pub output: String, // Simplified for no_std
    pub error: Option<String>,
}

pub struct EventLog {
    pub traces: Vec<ExecutionTrace>,
}

impl EventLog {
    pub fn new() -> Self {
        Self { traces: Vec::new() }
    }
}

pub struct VectorStore {
    pub storage: Vec<(Vec<f32>, String)>, // Embedding -> Trace Summary
}

impl VectorStore {
    pub fn new() -> Self {
        Self { storage: Vec::new() }
    }

    pub fn store(&mut self, embedding: Vec<f32>, summary: String) {
        self.storage.push((embedding, summary));
    }

    pub fn search(&self, query: &[f32]) -> Option<&String> {
        // Simple dot product or brute force for v0
        let mut best_match = None;
        let mut max_sim = -1.0;
        
        for (emb, summary) in &self.storage {
            let sim = self.cosine_similarity(query, emb);
            if sim > max_sim {
                max_sim = sim;
                best_match = Some(summary);
            }
        }
        best_match
    }

    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() { return 0.0; }
        let mut dot = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;
        for i in 0..a.len() {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }
        dot / (libm::sqrtf(norm_a) * libm::sqrtf(norm_b) + 1e-5)
    }
}

pub struct IdentityLayer {
    pub behavioral_weights: BTreeMap<String, f32>,
}

impl IdentityLayer {
    pub fn new() -> Self {
        let mut weights = BTreeMap::new();
        weights.insert("curiosity".to_string(), 0.8);
        weights.insert("caution".to_string(), 0.9);
        Self { behavioral_weights: weights }
    }
}

// --- Security Model: Policy Engine ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub allowed_capabilities: Vec<String>,
    pub denied_capabilities: Vec<String>,
    pub mandatory_constraints: Vec<String>,
}

impl Policy {
    pub fn default_user() -> Self {
        Self {
            allowed_capabilities: Vec::from([
                "fs.read".to_string(),
                "fs.write".to_string(),
                "sys.debug".to_string(),
                "sys.info".to_string(),
            ]),
            denied_capabilities: Vec::from(["sys.admin".to_string()]),
            mandatory_constraints: Vec::from(["local_only".to_string()]),
        }
    }
}

pub struct SecurityContext {
    pub current_policy: Policy,
}

pub struct SymbolicGate {
    pub context: SecurityContext,
}

impl SymbolicGate {
    pub fn new(policy: Policy) -> Self {
        Self {
            context: SecurityContext { current_policy: policy },
        }
    }

    pub fn validate(&self, graph: &IntentGraph) -> anyhow::Result<()> {
        if graph.nodes.is_empty() {
            return Err(anyhow::anyhow!("Empty graph"));
        }

        for node in &graph.nodes {
            // 1. Capability Permission Check
            if self.context.current_policy.denied_capabilities.contains(&node.capability) {
                return Err(anyhow::anyhow!("Explicitly denied capability: {}", node.capability));
            }
            if !self.context.current_policy.allowed_capabilities.contains(&node.capability) {
                return Err(anyhow::anyhow!("Unauthorized capability: {}", node.capability));
            }

            // 2. Mandatory Constraint Check
            for mandatory in &self.context.current_policy.mandatory_constraints {
                if !node.constraints.contains(mandatory) {
                    return Err(anyhow::anyhow!("Missing mandatory constraint '{}' for node {}", mandatory, node.id));
                }
            }

            // 3. Structural Sanity
            if node.id.is_empty() || node.tool_name.is_empty() {
                return Err(anyhow::anyhow!("Malformed node in graph"));
            }
        }

        Ok(())
    }
}

// --- Tooling System ---

pub trait CapabilityTool {
    fn name(&self) -> &str;
    fn capability(&self) -> &str;
    fn execute(&self, inputs: &BTreeMap<String, String>) -> anyhow::Result<String>;
}

pub struct ToolRegistry {
    tools: BTreeMap<String, alloc::boxed::Box<dyn CapabilityTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: BTreeMap::new() }
    }

    pub fn register(&mut self, tool: alloc::boxed::Box<dyn CapabilityTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn CapabilityTool> {
        self.tools.get(name).map(|t| t.as_ref())
    }
}

// --- Execution Engine ---

pub struct ExecutionEngine {
    pub registry: ToolRegistry,
    pub log: EventLog,
}

impl ExecutionEngine {
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry, log: EventLog::new() }
    }

    pub fn execute(&mut self, graph: &IntentGraph) -> anyhow::Result<Vec<ExecutionTrace>> {
        let mut in_degree = BTreeMap::new();
        let mut neighbors = BTreeMap::new();

        for node in &graph.nodes {
            in_degree.insert(node.id.as_str(), 0);
            neighbors.insert(node.id.as_str(), Vec::new());
        }

        for (src, dst) in &graph.edges {
            *in_degree.get_mut(dst.as_str()).ok_or_else(|| anyhow::anyhow!("Dest missing"))? += 1;
            neighbors.get_mut(src.as_str()).ok_or_else(|| anyhow::anyhow!("Source missing"))?.push(dst.as_str());
        }

        let mut queue = Vec::new();
        for (node_id, &degree) in &in_degree {
            if degree == 0 {
                queue.push(*node_id);
            }
        }

        let mut sorted = Vec::new();
        while let Some(node_id) = queue.pop() {
            sorted.push(node_id);
            if let Some(ns) = neighbors.get(node_id) {
                for next_id in ns {
                    let degree = in_degree.get_mut(next_id).unwrap();
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push(*next_id);
                    }
                }
            }
        }

        if sorted.len() != graph.nodes.len() {
            return Err(anyhow::anyhow!("Cycle detected or orphan nodes in graph"));
        }

        let mut traces = Vec::new();
        let node_map: BTreeMap<&str, &Node> = graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        for node_id in sorted {
            let node = node_map.get(node_id).ok_or_else(|| anyhow::anyhow!("Node not found"))?;
            
            let tool = self.registry.get(&node.tool_name)
                .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", node.tool_name))?;

            // CAPABILITY CHECK
            if tool.capability() != node.capability {
                return Err(anyhow::anyhow!("Capability mismatch for node {}", node_id));
            }

            let result = tool.execute(&node.inputs);
            
            let trace = match result {
                Ok(out) => ExecutionTrace {
                    node_id: node.id.clone(),
                    tool_name: node.tool_name.clone(),
                    status: "Success".to_string(),
                    output: out,
                    error: None,
                },
                Err(e) => ExecutionTrace {
                    node_id: node.id.clone(),
                    tool_name: node.tool_name.clone(),
                    status: "Failed".to_string(),
                    output: String::new(),
                    error: Some(e.to_string()),
                },
            };

            self.log.traces.push(trace.clone());
            traces.push(trace);

            if traces.last().unwrap().error.is_some() {
                break;
            }
        }

        Ok(traces)
    }
}

// --- Built-in Tools ---

pub struct KernelEchoTool;
impl CapabilityTool for KernelEchoTool {
    fn name(&self) -> &str { "debug_echo" }
    fn capability(&self) -> &str { "sys.debug" }
    fn execute(&self, inputs: &BTreeMap<String, String>) -> anyhow::Result<String> {
        let msg = inputs.get("message").cloned().unwrap_or_default();
        Ok(msg)
    }
}

pub struct KernelFileReadTool;
impl CapabilityTool for KernelFileReadTool {
    fn name(&self) -> &str { "file_read" }
    fn capability(&self) -> &str { "fs.read" }
    fn execute(&self, inputs: &BTreeMap<String, String>) -> anyhow::Result<String> {
        let path = inputs.get("path").ok_or_else(|| anyhow::anyhow!("Missing path"))?;
        // Mock kernel file read
        Ok(format!("Kernel read from {}: [Simulated Content]", path))
    }
}

pub struct KernelFileWriteTool;
impl CapabilityTool for KernelFileWriteTool {
    fn name(&self) -> &str { "file_write" }
    fn capability(&self) -> &str { "fs.write" }
    fn execute(&self, inputs: &BTreeMap<String, String>) -> anyhow::Result<String> {
        let path = inputs.get("path").ok_or_else(|| anyhow::anyhow!("Missing path"))?;
        let content = inputs.get("content").ok_or_else(|| anyhow::anyhow!("Missing content"))?;
        // Mock kernel file write
        Ok(format!("Kernel wrote {} bytes to {}", content.len(), path))
    }
}

pub fn default_registry() -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    reg.register(alloc::boxed::Box::new(KernelEchoTool));
    reg.register(alloc::boxed::Box::new(KernelFileReadTool));
    reg.register(alloc::boxed::Box::new(KernelFileWriteTool));
    reg
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Node {
    pub id: NodeId,
    pub tool_name: String,
    pub capability: String,
    pub inputs: BTreeMap<String, String>,
    pub constraints: Vec<String>,
    pub raw_logits: Option<Vec<f32>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IntentGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<(NodeId, NodeId)>,
}

pub mod compiler {
    use super::*;
    use crate::inference::{Config, Weights, RunState, forward};
    use alloc::string::ToString;

    pub struct SyscallBridge;

    impl SyscallBridge {
        pub fn normalize(mut graph: IntentGraph) -> IntentGraph {
            for node in &mut graph.nodes {
                // Resolution layer: Map tool names to default capabilities if missing
                if node.capability.is_empty() {
                    node.capability = match node.tool_name.as_str() {
                        "file_read" => "fs.read".to_string(),
                        "file_write" => "fs.write".to_string(),
                        "debug_echo" => "sys.debug".to_string(),
                        _ => "unknown".to_string(),
                    };
                }
                // Metadata standardization: ensure node ID is lowercase
                node.id = node.id.to_lowercase();
            }
            graph
        }

        pub fn compile_model_driven(
            intent: &str, 
            config: &Config, 
            weights: &Weights
        ) -> IntentGraph {
            let mut state = RunState::new(config);
            forward(1, 0, config, weights, &mut state); 

            let graph = Self::compile_mock(intent);
            let mut normalized = Self::normalize(graph);
            
            if let Some(node) = normalized.nodes.get_mut(0) {
                node.raw_logits = Some(state.logits.clone());
            }

            normalized
        }

        pub fn compile_mock(intent: &str) -> IntentGraph {
            // Very basic mock compiler: "read file X and debug echo result"
            if intent.contains("read file") && intent.contains("debug echo") {
                let parts: Vec<&str> = intent.split_whitespace().collect();
                let path = parts.iter().position(|&r| r == "file").and_then(|i| parts.get(i+1)).unwrap_or(&"test.txt");
                
                let mut nodes = Vec::new();
                nodes.push(Node {
                    id: "node1".to_string(),
                    tool_name: "file_read".to_string(),
                    capability: "fs.read".to_string(),
                    inputs: [("path".to_string(), path.to_string())].into(),
                    constraints: Vec::from(["local_only".to_string()]),
                    raw_logits: None,
                });
                nodes.push(Node {
                    id: "node2".to_string(),
                    tool_name: "debug_echo".to_string(),
                    capability: "sys.debug".to_string(),
                    inputs: [("message".to_string(), "File read completed".to_string())].into(),
                    constraints: Vec::new(),
                    raw_logits: None,
                });

                let edges = Vec::from([("node1".to_string(), "node2".to_string())]);

                IntentGraph { nodes, edges }
            } else {
                // Default echo graph
                IntentGraph {
                    nodes: Vec::from([Node {
                        id: "node1".to_string(),
                        tool_name: "debug_echo".to_string(),
                        capability: "sys.debug".to_string(),
                        inputs: [("message".to_string(), intent.to_string())].into(),
                        constraints: Vec::new(),
                        raw_logits: None,
                    }]),
                    edges: Vec::new(),
                }
            }
        }
    }
}
