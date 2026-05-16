# Danger OS

Danger OS is an experimental research operating system designed to treat natural language as a first-class execution primitive. It compiles natural language intents into **Typed Intent DAGs**, which are executed deterministically on a bare-metal, capability-based runtime.

## 🧠 Cognitive Architecture

Danger OS is not a chatbot; it is a **Cognitive Operating System**.

- **Language as IR**: High-level intents are compiled into a deterministic Intermediate Representation (IntentGraph).
- **Symbolic Gate**: Every graph is validated for safety, cycles, and capability requirements before execution.
- **Bare-Metal Inference**: The kernel bundles an LLM (Gemma 4 e2b) and includes a `no_std` inference engine to process intents without a host OS.

## 🏗 Repository Structure

- `crates/kernel`: The `no_std` x86_64 kernel.
  - **Memory**: Custom physical memory mapper and `linked_list_allocator` heap.
  - **Inference**: Minimalist Llama-architecture inference engine.
  - **I/O**: UART 16550 Serial and VGA Framebuffer support.
- `crates/runner`: Host-side orchestration tool.
  - Downloads model weights.
  - Packages the kernel and LLM bundle into a UEFI-bootable disk image.
  - Launches QEMU with appropriate hardware acceleration and memory.
- `crates/runtime`: Core execution logic for IntentGraphs (Transitioning to kernel).
- `crates/tools`: The "Capability Tools" (e.g., file I/O, debug echo) available to the OS.

## 🚀 Getting Started

### Prerequisites
- **Rust Nightly**: Required for `no_std` and `bindeps`.
- **QEMU**: `qemu-system-x86_64` for emulation.
- **OVMF**: Required for UEFI booting.

### Build and Boot
```bash
cargo +nightly run -Z bindeps -p danger-runner
```
The runner will automatically:
1. Compile the kernel for `x86_64-unknown-none`.
2. Download the verification weights (Gemma 4 proxy).
3. Generate a `danger-os-uefi.img`.
4. Launch QEMU and pipe the kernel's cognitive boot sequence to your terminal.

## 🛠 Design Philosophy
Computation in Danger OS is expressed as validated execution graphs. The LLM is an **untrusted frontend compiler** that produces structured intents. The kernel acts as the **trusted execution environment**, ensuring that every operation conforms to declared capabilities and safety policies.
