#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use bootloader_api::{entry_point, BootInfo, config::Mapping};
use core::panic::PanicInfo;
use uart_16550::SerialPort;
use core::fmt::Write;
use x86_64::VirtAddr;
use danger_kernel::{memory, allocator, inference, DangerOS};

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

    let _ = writeln!(serial_port, "[OK] System Heap Active (10MB)");

    // 2. Load TinyLlama Bundle (Ramdisk)
    if let Some(ramdisk_addr) = boot_info.ramdisk_addr.into_option() {
        let ramdisk_slice = unsafe {
            core::slice::from_raw_parts(ramdisk_addr as *const u8, boot_info.ramdisk_len as usize)
        };

        // 3. Inference Engine
        let _ = writeln!(serial_port, "Parsing TinyLlama Architecture...");
        let (config, weights) = inference::parse_model(ramdisk_slice);
        let _ = writeln!(serial_port, "[OK] Config: {:?}", config);

        // 4. Initialize Danger OS Substrate
        let mut system = DangerOS::new();

        let _ = writeln!(serial_port, "\n\x1b[1;35m╭──────────────────────────────────────────────────────────╮\x1b[0m");
        let _ = writeln!(serial_port, "\x1b[1;35m│\x1b[0m \x1b[1;36mDANGER OS v0.1\x1b[0m — \x1b[1;32mCognitive Substrate Initialized\x1b[0m       \x1b[1;35m│\x1b[0m");
        let _ = writeln!(serial_port, "\x1b[1;35m│\x1b[0m Kernel: \x1b[33mno_std/x86_64\x1b[0m | Memory: \x1b[33mActive\x1b[0m                \x1b[1;35m│\x1b[0m");
        let _ = writeln!(serial_port, "\x1b[1;35m╰──────────────────────────────────────────────────────────╯\x1b[0m");
        let _ = writeln!(serial_port, "\n\x1b[2mType a natural language intent or 'help'.\x1b[0m\n");

        let mut input_buffer = alloc::string::String::new();
        loop {
            let _ = write!(serial_port, "\x1b[1;38;5;208m╭─\x1b[0m \x1b[1;32mdanger-os\x1b[0m \x1b[1;38;5;208m─╮\x1b[0m\n\x1b[1;38;5;208m╰─\x1b[0m \x1b[1;36mλ\x1b[0m ");
            input_buffer.clear();
            
            loop {
                let byte = serial_port.receive();
                match byte {
                    b'\r' | b'\n' => { let _ = writeln!(serial_port); break; }
                    8 | 127 => { if !input_buffer.is_empty() { input_buffer.pop(); let _ = write!(serial_port, "\x08 \x08"); } }
                    32..=126 => { input_buffer.push(byte as char); let _ = write!(serial_port, "{}", byte as char); }
                    _ => {}
                }
            }

            if input_buffer.is_empty() { continue; }
            if input_buffer == "help" {
                let _ = writeln!(serial_port, "\x1b[1;34m[STATUS]\x1b[0m Allowed Caps: \x1b[32m{:?}\x1b[0m", system.gate.context.current_policy.allowed_capabilities);
                let _ = writeln!(serial_port, "         Semantic Memory: \x1b[33m{} entries\x1b[0m\n", system.vector_store.storage.len());
                continue;
            }

            let _ = write!(serial_port, "\x1b[1;30m[THINKING]\x1b[0m ");
            for _ in 0..3 { let _ = write!(serial_port, "\x1b[30m.\x1b[0m"); for _ in 0..1000000 { core::hint::spin_loop(); } }
            
            match system.process_intent(&input_buffer, &config, &weights) {
                Ok(traces) => {
                    let _ = writeln!(serial_port, " \x1b[32mOK\x1b[0m");
                    for trace in traces {
                        let color = if trace.status == "Success" { "\x1b[32m" } else { "\x1b[31m" };
                        let _ = writeln!(serial_port, "  \x1b[1;30m└─\x1b[0m {}●\x1b[0m \x1b[1m{}\x1b[0m: {}", color, trace.tool_name, trace.output);
                        if let Some(e) = trace.error { let _ = writeln!(serial_port, "     \x1b[31m╰─ error: {}\x1b[0m", e); }
                    }
                }
                Err(e) => {
                    let _ = writeln!(serial_port, " \x1b[31mREJECTED\x1b[0m");
                    let _ = writeln!(serial_port, "  \x1b[1;31m[ERROR]\x1b[0m {}", e);
                }
            }
            let _ = writeln!(serial_port);
        }
    } else {
        let _ = writeln!(serial_port, "[CRITICAL] TinyLlama Bundle not found!");
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
