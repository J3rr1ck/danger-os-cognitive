use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolSchema {
    pub inputs: HashMap<String, String>, // key -> type
    pub outputs: HashMap<String, String>,
}

#[async_trait]
pub trait CapabilityTool: Send + Sync {
    fn name(&self) -> &str;
    fn schema(&self) -> ToolSchema;
    async fn execute(&self, input: serde_json::Value) -> Result<serde_json::Value>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn CapabilityTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn CapabilityTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn CapabilityTool> {
        self.tools.get(name).map(|t| t.as_ref())
    }
}

// --- Sample Tools ---

pub struct DebugEchoTool;

#[async_trait]
impl CapabilityTool for DebugEchoTool {
    fn name(&self) -> &str { "debug_echo" }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            inputs: [("message".to_string(), "string".to_string())].into(),
            outputs: [("echo".to_string(), "string".to_string())].into(),
        }
    }
    async fn execute(&self, input: serde_json::Value) -> Result<serde_json::Value> {
        let message = input.get("message").and_then(|v| v.as_str()).unwrap_or("");
        println!("[TOOL:debug_echo] {}", message);
        Ok(serde_json::json!({ "echo": message }))
    }
}

pub struct FileReadTool;

#[async_trait]
impl CapabilityTool for FileReadTool {
    fn name(&self) -> &str { "file_read" }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            inputs: [("path".to_string(), "string".to_string())].into(),
            outputs: [("content".to_string(), "string".to_string())].into(),
        }
    }
    async fn execute(&self, input: serde_json::Value) -> Result<serde_json::Value> {
        let path = input.get("path").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing path"))?;
        // Mock read for prototype
        let content = format!("Mock content for {}", path);
        println!("[TOOL:file_read] Reading {}", path);
        Ok(serde_json::json!({ "content": content }))
    }
}

pub struct FileWriteTool;

#[async_trait]
impl CapabilityTool for FileWriteTool {
    fn name(&self) -> &str { "file_write" }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            inputs: [
                ("path".to_string(), "string".to_string()),
                ("content".to_string(), "string".to_string())
            ].into(),
            outputs: [("status".to_string(), "string".to_string())].into(),
        }
    }
    async fn execute(&self, input: serde_json::Value) -> Result<serde_json::Value> {
        let path = input.get("path").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing path"))?;
        let content = input.get("content").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing content"))?;
        println!("[TOOL:file_write] Writing to {}: {}", path, content);
        Ok(serde_json::json!({ "status": "success" }))
    }
}

pub fn default_registry() -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(DebugEchoTool));
    reg.register(Box::new(FileReadTool));
    reg.register(Box::new(FileWriteTool));
    reg
}
