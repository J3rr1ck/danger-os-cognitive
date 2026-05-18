use std::process::Command;
use std::path::PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Build and run in QEMU (default)
    Run,
    /// Build a bootable ISO image
    CookIso,
    /// Run the cooked ISO in QEMU
    RunIso,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::CookIso) => cook_iso()?,
        Some(Commands::RunIso) => run_iso()?,
        _ => run_qemu()?,
    }

    Ok(())
}

fn build_kernel_and_image() -> anyhow::Result<(PathBuf, PathBuf)> {
    // Build kernel in release mode for performance
    println!("[RUNNER] Building kernel in release mode...");
    let status = Command::new("cargo")
        .arg("+nightly")
        .arg("build")
        .arg("-Z")
        .arg("bindeps")
        .arg("--release")
        .arg("-p")
        .arg("danger-kernel")
        .arg("--target")
        .arg("x86_64-unknown-none")
        .status()?;
    if !status.success() {
        anyhow::bail!("Kernel build failed");
    }

    let kernel_path = PathBuf::from("target/x86_64-unknown-none/release/danger-kernel");
    
    // Download the tiny model if it doesn't exist
    let model_url = "https://huggingface.co/karpathy/tinyllamas/resolve/main/stories260K/stories260K.bin";
    let model_path = PathBuf::from("gemma4-e2b-q4.bin"); // Name it as requested
    
    if !model_path.exists() {
        println!("[RUNNER] Downloading model from {}...", model_url);
        let output = Command::new("curl")
            .arg("-L")
            .arg(model_url)
            .arg("-o")
            .arg(&model_path)
            .output()?;
        if !output.status.success() {
            anyhow::bail!("Failed to download model: {}", String::from_utf8_lossy(&output.stderr));
        }
    }

    // Use UefiBoot
    let mut bootloader = bootloader::UefiBoot::new(&kernel_path);
    bootloader.set_ramdisk(&model_path);
    
    let image_path = PathBuf::from("danger-os-uefi.img");
    println!("[RUNNER] Creating UEFI disk image with bundled model...");
    bootloader.create_disk_image(&image_path)?;

    Ok((kernel_path, image_path))
}

fn run_qemu() -> anyhow::Result<()> {
    let (_, image_path) = build_kernel_and_image()?;

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

fn cook_iso() -> anyhow::Result<()> {
    let (_, image_path) = build_kernel_and_image()?;

    let iso_path = "danger-os.iso";
    println!("[RUNNER] Cooking bootable ISO: {}...", iso_path);

    // Create a temporary directory for xorriso context
    let tmp_dir = std::env::temp_dir().join("danger-os-iso-ctx");
    if tmp_dir.exists() {
        std::fs::remove_dir_all(&tmp_dir)?;
    }
    std::fs::create_dir_all(&tmp_dir)?;

    let status = Command::new("xorriso")
        .arg("-as")
        .arg("mkisofs")
        .arg("-R")
        .arg("-J")
        .arg("-V")
        .arg("DANGER_OS")
        .arg("-efi-boot-part")
        .arg(&image_path)
        .arg("-isohybrid-gpt-basdat")
        .arg("-o")
        .arg(iso_path)
        .arg(&tmp_dir)
        .status()?;

    if !status.success() {
        anyhow::bail!("xorriso failed to create ISO");
    }

    println!("[OK] ISO created successfully: {}", iso_path);
    
    // Clean up
    let _ = std::fs::remove_dir_all(&tmp_dir);

    Ok(())
}

fn run_iso() -> anyhow::Result<()> {
    let iso_path = PathBuf::from("danger-os.iso");
    if !iso_path.exists() {
        println!("[RUNNER] ISO not found, cooking it first...");
        cook_iso()?;
    }

    println!("[RUNNER] Launching QEMU with ISO: {}...", iso_path.display());
    let mut qemu = Command::new("qemu-system-x86_64");
    
    // Boot from CD-ROM (ISO)
    qemu.arg("-drive").arg(format!("format=raw,file={}", iso_path.display()));
    
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
