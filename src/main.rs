use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
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
    loop {
        let n = match response.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
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
    let asset_name = "kopi-0.1.0-alpha.tar.gz";
    let download_url = "https://github.com/kinoite/kopi-lang/releases/download/0.1.0/kopi-0.1.0-alpha.tar.gz";

    println!("Downloading from: {}", download_url);

    let temp_dir = env::temp_dir().join("kipper_install");
    fs::create_dir_all(&temp_dir)?;
    let archive_path = temp_dir.join(asset_name);

    download_file(download_url, &archive_path)?;
    
    println!("\nUnpacking toolchain...");
    unpack_archive(&archive_path, &kopi_bin)?;
    
    println!("Installation complete.");
    
    update_shell_profile(kopi_bin.to_str().unwrap())?;

    println!("\nTo get started, please restart your terminal or run:");
    println!("  source ~/.zshrc   (or ~/.bashrc, ~/.profile)");

    Ok(())
}
