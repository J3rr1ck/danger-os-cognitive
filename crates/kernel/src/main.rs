#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use bootloader_api::{entry_point, BootInfo, config::Mapping};
use core::panic::PanicInfo;
use uart_16550::SerialPort;
use core::fmt::Write;
use x86_64::VirtAddr;
use danger_kernel::{memory, allocator, inference, compiler::SyscallBridge, SymbolicGate};

pub static BOOTLOADER_CONFIG: bootloader_api::config::BootloaderConfig = {
    let mut config = bootloader_api::config::BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic); // Ensure physical memory is mapped
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    let mut serial_port = unsafe { SerialPort::new(0x3F8) };
    serial_port.init();
    
    let _ = writeln!(serial_port, "Danger OS: Starting Cognitive Boot Sequence...");

    // 1. Memory Initialization
    let phys_mem_offset_val = boot_info.physical_memory_offset.into_option()
        .expect("Physical memory offset not provided. Ensure BOOTLOADER_CONFIG is correct.");
    let phys_mem_offset = VirtAddr::new(phys_mem_offset_val);
    
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        memory::BootInfoFrameAllocator::init(&boot_info.memory_regions)
    };

    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    let _ = writeln!(serial_port, "[OK] System Heap Active (1MB)");

    // 2. Load Gemma 4 Bundle (Ramdisk)
    if let Some(ramdisk_addr) = boot_info.ramdisk_addr.into_option() {
        let ramdisk_len = boot_info.ramdisk_len;
        let _ = writeln!(serial_port, "[OK] Found TinyLlama Bundle at {:#x} ({} bytes)", ramdisk_addr, ramdisk_len);

        let ramdisk_slice = unsafe {
            core::slice::from_raw_parts(ramdisk_addr as *const u8, ramdisk_len as usize)
        };

        // 3. Inference Engine
        let _ = writeln!(serial_port, "Parsing TinyLlama Architecture...");
        let (config, weights) = inference::parse_model(ramdisk_slice);
        let _ = writeln!(serial_port, "[OK] Config: {:?}", config);

        // 4. Cognitive REPL (Claude Code Style)
        let mut registry = danger_kernel::default_registry();
        let mut engine = danger_kernel::ExecutionEngine::new(registry);
        let gate = SymbolicGate::new();

        let _ = writeln!(serial_port, "\n\x1b[1;35m╭──────────────────────────────────────────────────────────╮\x1b[0m");
        let _ = writeln!(serial_port, "\x1b[1;35m│\x1b[0m \x1b[1;36mDANGER OS v0.1\x1b[0m — \x1b[1;32mCognitive Substrate Initialized\x1b[0m       \x1b[1;35m│\x1b[0m");
        let _ = writeln!(serial_port, "\x1b[1;35m│\x1b[0m Kernel: \x1b[33mno_std/x86_64\x1b[0m | Model: \x1b[33mTinyLlama-260K\x1b[0m       \x1b[1;35m│\x1b[0m");
        let _ = writeln!(serial_port, "\x1b[1;35m╰──────────────────────────────────────────────────────────╯\x1b[0m");
        let _ = writeln!(serial_port, "\n\x1b[2mType a natural language intent (e.g., 'read file test.txt') or 'help'.\x1b[0m\n");

        let mut input_buffer = alloc::string::String::new();
        loop {
            let _ = write!(serial_port, "\x1b[1;38;5;208m╭─\x1b[0m \x1b[1;32mdanger-os\x1b[0m \x1b[1;38;5;208m─╮\x1b[0m\n\x1b[1;38;5;208m╰─\x1b[0m \x1b[1;36mλ\x1b[0m ");
            input_buffer.clear();
            
            // Basic serial input loop
            loop {
                let byte = serial_port.receive();
                match byte {
                    b'\r' | b'\n' => {
                        let _ = writeln!(serial_port);
                        break;
                    }
                    8 | 127 => { // Backspace
                        if !input_buffer.is_empty() {
                            input_buffer.pop();
                            let _ = write!(serial_port, "\x08 \x08");
                        }
                    }
                    32..=126 => {
                        input_buffer.push(byte as char);
                        let _ = write!(serial_port, "{}", byte as char);
                    }
                    _ => {}
                }
            }

            if input_buffer.is_empty() { continue; }
            if input_buffer == "help" {
                let _ = writeln!(serial_port, "\x1b[1;34m[HELP]\x1b[0m Available tools: \x1b[33mdebug_echo, file_read, file_write\x1b[0m");
                let _ = writeln!(serial_port, "      Capabilities: \x1b[33msys.debug, fs.read, fs.write\x1b[0m\n");
                continue;
            }

            let _ = write!(serial_port, "\x1b[1;30m[THINKING]\x1b[0m ");
            // Simulated model output delay / tokens
            for _ in 0..3 {
                let _ = write!(serial_port, "\x1b[30m.\x1b[0m");
                for _ in 0..1000000 { core::hint::spin_loop(); }
            }
            
            let graph = SyscallBridge::compile_model_driven(&input_buffer, &config, &weights);
            let _ = writeln!(serial_port, " \x1b[32mOK\x1b[0m");
            
            match gate.validate(&graph) {
                Ok(_) => {
                    let _ = writeln!(serial_port, "\x1b[1;34m[PLAN]\x1b[0m Generated DAG with \x1b[33m{}\x1b[0m nodes.", graph.nodes.len());
                    match engine.execute(&graph) {
                        Ok(traces) => {
                            for trace in traces {
                                let color = if trace.status == "Success" { "\x1b[32m" } else { "\x1b[31m" };
                                let _ = writeln!(serial_port, "  \x1b[1;30m└─\x1b[0m {}●\x1b[0m \x1b[1m{}\x1b[0m: {}", color, trace.tool_name, trace.output);
                                if let Some(e) = trace.error {
                                    let _ = writeln!(serial_port, "     \x1b[31m╰─ error: {}\x1b[0m", e);
                                }
                            }
                        }
                        Err(e) => {
                            let _ = writeln!(serial_port, "\x1b[1;31m[EXECUTION FAILURE]\x1b[0m {}", e);
                        }
                    }
                }
                Err(e) => {
                    let _ = writeln!(serial_port, "\x1b[1;31m[SAFETY REJECTION]\x1b[0m {}", e);
                }
            }
            let _ = writeln!(serial_port);
        }
    } else {
        let _ = writeln!(serial_port, "[CRITICAL] TinyLlama Bundle not found in ramdisk!");
    }

    let _ = writeln!(serial_port, "Danger OS: System Ready.");

    loop {}
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let mut serial_port = unsafe { SerialPort::new(0x3F8) };
    serial_port.init();
    let _ = writeln!(serial_port, "KERNEL PANIC: {:?}", info);
    loop {}
}
