#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use bootloader_api::{entry_point, BootInfo, config::Mapping};
use core::panic::PanicInfo;
use uart_16550::SerialPort;
use core::fmt::Write;
use x86_64::VirtAddr;

pub mod memory;
pub mod allocator;
pub mod inference;

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
        let _ = writeln!(serial_port, "[OK] Found Gemma 4 Bundle at {:#x} ({} bytes)", ramdisk_addr, ramdisk_len);

        let ramdisk_slice = unsafe {
            core::slice::from_raw_parts(ramdisk_addr as *const u8, ramdisk_len as usize)
        };

        // 3. Inference Engine
        let _ = writeln!(serial_port, "Parsing Gemma 4 Architecture...");
        let (config, weights) = inference::parse_model(ramdisk_slice);
        let _ = writeln!(serial_port, "[OK] Config: {:?}", config);

        let _ = writeln!(serial_port, "Executing Cognitive Hello World...");
        let result = inference::run_inference(&config, &weights);
        
        let _ = writeln!(serial_port, "\n>>> INFERENCE RESULT: {}\n", result);
    } else {
        let _ = writeln!(serial_port, "[CRITICAL] Gemma 4 Bundle not found in ramdisk!");
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
