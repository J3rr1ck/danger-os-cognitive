use clap::Parser;
use danger_kernel::{SymbolicGate, compiler::SyscallBridge};
use danger_runtime::ExecutionEngine;
use danger_tools::default_registry;
use anyhow::Result;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Mocked natural language intent
    #[arg(short, long, default_value = "read file test.txt and debug echo result")]
    intent: String,

    /// Show the generated graph
    #[arg(short, long)]
    show_graph: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("--- Danger OS Cognitive Prototype ---");
    println!("Intent: \"{}\"", args.intent);

    // 1. Compilation (Mock)
    println!("\n[1] Compiling intent into IntentGraph...");
    let graph = SyscallBridge::compile_mock(&args.intent);

    if args.show_graph {
        println!("Generated Graph: {}", serde_json::to_string_pretty(&graph)?);
    }

    // 2. Validation (Symbolic Gate)
    println!("[2] Validating graph safety via Symbolic Gate...");
    let gate = SymbolicGate::new();
    gate.validate(&graph)?;
    println!("Validation successful.");

    // 3. Execution
    println!("[3] Executing graph via Runtime Engine...");
    let registry = default_registry();
    let engine = ExecutionEngine::new(registry);
    
    match engine.execute(graph).await {
        Ok(traces) => {
            println!("\n--- Execution Trace ---");
            for trace in traces {
                println!(
                    "[{}] Node: {} | Status: {} | Duration: {}ms",
                    trace.node_id, trace.tool_name, trace.status, trace.duration_ms
                );
                if let Some(output) = trace.output {
                    println!("  Output: {}", output);
                }
            }
            println!("\nSystem halted successfully.");
        }
        Err(e) => {
            eprintln!("\n[CRITICAL] System failure: {}", e);
        }
    }

    Ok(())
}
