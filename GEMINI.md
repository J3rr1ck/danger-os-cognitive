# Danger OS: Agent Instructions

This file contains foundational mandates and architectural context for Gemini CLI when operating on the Danger OS codebase.

## 🛠 Toolchain & Environment
- **Target**: `x86_64-unknown-none` (Bare Metal).
- **Toolchain**: `nightly` is mandatory.
- **Unstable Features**: Always use `-Z bindeps` when building via the runner.
- **Bootloader**: We use `bootloader_api` v0.11 with UEFI booting.

## 🏗 Architectural Mandates
1. **no_std strictness**: The `crates/kernel` must remain `#![no_std]`. Avoid bringing in `std` dependencies.
2. **Memory Management**: 
   - Heap is managed by `linked_list_allocator`. 
   - Initial heap size is currently limited to 10MB to ensure stability on varying QEMU setups; increase only if the model requires it.
3. **Inference Engine**:
   - The engine in `inference.rs` is a minimalist Llama-based parser. 
   - Model weights are bundled as a `ramdisk` module via the bootloader.
4. **Runner Logic**:
   - `crates/runner` is a standard `std` crate used for orchestration. 
   - It is responsible for fetching model weights and building the final `.img`.

## 🧬 Memory Map & Constants
- **Heap Start**: `0x_4444_4444_0000`
- **Serial Port**: `0x3F8` (COM1)
- **Model Bundle**: Located via `boot_info.ramdisk_addr`.

## 📜 Development Workflow
- To test booting: `cargo +nightly run -Z bindeps -p danger-runner`.
- For kernel changes: Ensure `crates/kernel/Cargo.toml` dependencies are compatible with `no_std` and `alloc`.
- For model updates: Modify the `model_url` in `crates/runner/src/main.rs`.
