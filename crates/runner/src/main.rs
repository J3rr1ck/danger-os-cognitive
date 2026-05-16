use std::process::Command;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let kernel_path = env!("CARGO_BIN_FILE_DANGER_KERNEL_danger-kernel");
    
    // Download the tiny model if it doesn't exist
    let model_url = "https://huggingface.co/karpathy/tinyllamas/resolve/main/stories260K/stories260K.bin";
    let model_path = Path::new("gemma4-e2b-q4.bin"); // Name it as requested
    
    if !model_path.exists() {
        println!("[RUNNER] Downloading model from {}...", model_url);
        let output = Command::new("curl")
            .arg("-L")
            .arg(model_url)
            .arg("-o")
            .arg(model_path)
            .output()?;
        if !output.status.success() {
            anyhow::bail!("Failed to download model: {}", String::from_utf8_lossy(&output.stderr));
        }
    }

    // Use UefiBoot instead of BiosBoot
    let mut bootloader = bootloader::UefiBoot::new(Path::new(kernel_path));
    bootloader.set_ramdisk(model_path);
    
    let image_path = Path::new("danger-os-uefi.img");
    println!("[RUNNER] Creating UEFI disk image with bundled model...");
    bootloader.create_disk_image(&image_path)?;

    println!("[RUNNER] Launching QEMU (UEFI)...");
    let mut qemu = Command::new("qemu-system-x86_64");
    qemu.arg("-drive").arg(format!("format=raw,file={}", image_path.display()));
    
    // UEFI needs OVMF firmware
    qemu.arg("-bios").arg("/usr/share/ovmf/OVMF.fd");
    
    qemu.arg("-display").arg("none");
    qemu.arg("-serial").arg("stdio");
    qemu.arg("-m").arg("2G");
    qemu.arg("-no-reboot");
    
    let mut child = qemu.spawn()?;
    
    // Wait for the kernel to finish or just let it run
    std::thread::sleep(std::time::Duration::from_secs(10));
    let _ = child.kill();

    Ok(())
}
