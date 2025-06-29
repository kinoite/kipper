use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle, HumanDuration};
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use anyhow::{Context, Result};

#[derive(Parser, Debug)]
#[command(name = "kipper", version, about = "The installer for the Kopi Language toolchain")]
struct Cli {}

fn get_target_triple() -> &'static str {
    match (env::consts::OS, env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        (os, arch) => panic!("Unsupported OS/architecture combination: {}/{}", os, arch),
    }
}

fn download_file(url: &str, path: &Path) -> Result<()> {
    let mut response = reqwest::blocking::get(url)?.error_for_status()?;
    let total_size = response.content_length().unwrap_or(0);

    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")?
        .progress_chars("#>-"));

    let mut dest = File::create(path)?;
    let mut downloaded: u64 = 0;
    let mut buffer = [0; 8192];
    
    while let Ok(n) = response.read(&mut buffer) {
        if n == 0 { break; }
        dest.write_all(&buffer[..n])?;
        downloaded += n as u64;
        pb.set_position(downloaded);
    }

    pb.finish_with_message("Download complete");
    Ok(())
}

fn unpack_archive(archive_path: &Path, dest_path: &Path) -> Result<()> {
    let tar_gz = File::open(archive_path)?;
    let tar = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(tar);
    archive.unpack(dest_path)?;
    Ok(())
}

fn update_shell_profile(kopi_bin_dir: &str) -> Result<()> {
    let shell_profile_path = match env::var("SHELL").as_deref() {
        Ok(shell) if shell.contains("zsh") => Some(PathBuf::from(env::var("HOME")?).join(".zshrc")),
        Ok(shell) if shell.contains("bash") => Some(PathBuf::from(env::var("HOME")?).join(".bashrc")),
        _ => Some(PathBuf::from(env::var("HOME")?).join(".profile")),
    };

    if let Some(path) = shell_profile_path {
        if !path.exists() {
            File::create(&path)?;
        }
        let content = fs::read_to_string(&path)?;
        let export_cmd = format!("export PATH=\"{}:$PATH\"", kopi_bin_dir);

        if !content.contains(&export_cmd) {
            let mut file = fs::OpenOptions::new().append(true).open(&path)?;
            writeln!(file, "\n# Kopi Language Environment")?;
            writeln!(file, "{}", export_cmd)?;
            println!("Kopi PATH added to {}.", path.display());
        } else {
            println!("Kopi is already in your PATH.");
        }
    } else {
        println!("Could not detect shell profile. Please add the following to your PATH manually:");
        println!("  {}", format!("export PATH=\"{}:$PATH\"", kopi_bin_dir));
    }
    Ok(())
}

fn main() -> Result<()> {
    let _cli = Cli::parse();
    println!("Welcome to Kipper, the Kopi installer!");
    
    let home_dir = env::var("HOME").context("Failed to get HOME directory")?;
    let kopi_home = PathBuf::from(home_dir).join(".kopi");
    let kopi_bin = kopi_home.join("bin");
    fs::create_dir_all(&kopi_bin).context("Failed to create Kopi installation directory")?;
    let temp_dir = env::temp_dir().join("kipper_install");
    if temp_dir.exists() { fs::remove_dir_all(&temp_dir)?; }
    fs::create_dir_all(&temp_dir)?;

    let version_tag = "v0.1.0";
    let download_url = format!(
        "https://github.com/kinoite/kopi-lang/archive/refs/tags/{}.tar.gz",
        version_tag
    );
    let archive_path = temp_dir.join(format!("{}.tar.gz", version_tag));
    
    println!("Downloading Kopi source code (version {})", version_tag);
    download_file(&download_url, &archive_path)?;
    
    println!("\nUnpacking source code...");
    unpack_archive(&archive_path, &temp_dir)?;
    println!("Unpacked successfully.");

    let source_dir_name = format!("kopi-lang-{}", version_tag);
    let compile_path = temp_dir.join(source_dir_name).join("kopi_rust");
    
    let bar_style = ProgressStyle::default_spinner()
        .tick_strings(&[
            "==    ",
            " =    ",
            "  =   ",
            "   =  ",
            "    = ",
            "    ==",
            "    = ",
            "   =  ",
            "  =   ",
            " =    ",
        ])
        .template("{spinner:.red} {msg}")?;

    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(std::time::Duration::from_millis(120));
    pb.set_style(bar_style);
    pb.set_message("Compiling Kopi interpreter (this may take a moment)...");
    
    let start_time = Instant::now();
    let build_status = Command::new("cargo")
        .args(["build", "--release", "--quiet"])
        .current_dir(&compile_path)
        .output()
        .context("Failed to execute 'cargo'. Is the Rust toolchain installed?")?;

    if !build_status.status.success() {
        pb.finish_with_message("Compilation failed.");
        io::stderr().write_all(&build_status.stderr)?;
        return Err(anyhow::anyhow!("Cargo build failed."));
    }
    
    let duration = start_time.elapsed();
    pb.finish_with_message(format!("Compilation finished in {}.", HumanDuration(duration)));

    let compiled_binary_path = compile_path.join("target/release/kopi");
    let dest_path = kopi_bin.join("kopi");
    fs::rename(&compiled_binary_path, &dest_path)
        .context(format!("Failed to move compiled binary from {:?} to {:?}", compiled_binary_path, dest_path))?;

    println!("Successfully installed Kopi to {}", dest_path.display());
    
    update_shell_profile(kopi_bin.to_str().unwrap())?;

    println!("\nTo get started, please restart your terminal or run:");
    println!("  source ~/.zshrc   (or ~/.bashrc, ~/.profile)");

    Ok(())
}
