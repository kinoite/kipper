// Kipper - The Kopi Language Installer
// A git-based installer for Kopi written in Rust

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const REPO_URL: &str = "https://github.com/kinoite/kopi-lang.git";
const INSTALLER_NAME: &str = "kipper";

#[derive(Debug)]
enum InstallerError {
    Io(io::Error),
    Git(String),
    Cargo(String),
    PathError(String),
}

impl From<io::Error> for InstallerError {
    fn from(error: io::Error) -> Self {
        InstallerError::Io(error)
    }
}

struct Installer {
    install_dir: PathBuf,
    bin_dir: PathBuf,
    temp_dir: PathBuf,
}

impl Installer {
    fn new() -> Result<Self, InstallerError> {
        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .map_err(|_| InstallerError::PathError("Could not determine home directory".to_string()))?;
        
        let install_dir = Path::new(&home).join(".kopi");
        let bin_dir = if cfg!(windows) {
            install_dir.clone()
        } else {
            Path::new(&home).join(".local").join("bin")
        };
        
        let temp_dir = env::temp_dir().join(format!("kopi-install-{}", std::process::id()));

        Ok(Installer {
            install_dir,
            bin_dir,
            temp_dir,
        })
    }

    fn print_banner(&self) {
        println!("\x1b[34mKipper - The Kopi Language Installer\x1b[0m");
        println!("\x1b[33mFast, modern, and lightweight scripting language\x1b[0m");
        println!();
    }

    fn log_info(&self, msg: &str) {
        println!("\x1b[34m[INFO]\x1b[0m {}", msg);
    }

    fn log_success(&self, msg: &str) {
        println!("\x1b[32m[YAY!]\x1b[0m {}", msg);
    }

    fn log_warning(&self, msg: &str) {
        println!("\x1b[33m[WARN]\x1b[0m {}", msg);
    }

    fn log_error(&self, msg: &str) {
        println!("\x1b[31m[ERR]\x1b[0m {}", msg);
    }

    fn check_dependencies(&self) -> Result<(), InstallerError> {
        self.log_info("Checking dependencies...");

        if !self.command_exists("git") {
            self.log_error("git is required but not installed");
            self.log_info("Please install git and try again");
            return Err(InstallerError::Git("git not found".to_string()));
        }
        
        if !self.command_exists("cargo") {
            self.log_error("Rust/Cargo is required but not installed");
            self.log_info("Please install Rust from https://rustup.rs/ and try again");
            return Err(InstallerError::Cargo("cargo not found".to_string()));
        }

        self.log_success("All dependencies found");
        Ok(())
    }

    fn command_exists(&self, cmd: &str) -> bool {
        Command::new(cmd)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    fn create_directories(&self) -> Result<(), InstallerError> {
        self.log_info("Creating installation directories...");
        fs::create_dir_all(&self.install_dir)?;
        fs::create_dir_all(&self.bin_dir)?;
        fs::create_dir_all(&self.temp_dir)?;
        Ok(())
    }

    fn download_and_build(&self) -> Result<(), InstallerError> {
        self.log_info("Downloading Kopi source code...");
        
        let clone_dir = self.temp_dir.join("kopi-lang");
        
        let output = Command::new("git")
            .args(&["clone", REPO_URL])
            .arg(&clone_dir)
            .output()?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(InstallerError::Git(format!("Failed to clone repository: {}", error)));
        }

        self.log_info("Building Kopi (this may take a few minutes)...");
        
        let build_output = Command::new("cargo")
            .args(&["build", "--release"])
            .current_dir(&clone_dir)
            .output()?;

        if !build_output.status.success() {
            let error = String::from_utf8_lossy(&build_output.stderr);
            return Err(InstallerError::Cargo(format!("Build failed: {}", error)));
        }

        let binary_name = if cfg!(windows) { "kopi.exe" } else { "kopi" };
        let binary_path = clone_dir.join("target").join("release").join(binary_name);
        
        if !binary_path.exists() {
            return Err(InstallerError::Cargo("Built binary not found".to_string()));
        }

        self.log_success("Build completed successfully");
        Ok(())
    }

    fn install_binary(&self) -> Result<(), InstallerError> {
        self.log_info("Installing Kopi binary...");
        
        let binary_name = if cfg!(windows) { "kopi.exe" } else { "kopi" };
        let source_path = self.temp_dir.join("kopi-lang").join("target").join("release").join(binary_name);
        let dest_path = self.install_dir.join(binary_name);
        
        fs::copy(&source_path, &dest_path)?;
        
        // On Unix-like systems, create a symlink in bin directory
        #[cfg(unix)]
        {
            let bin_path = self.bin_dir.join("kopi");
            if bin_path.exists() {
                fs::remove_file(&bin_path)?;
            }
            std::os::unix::fs::symlink(&dest_path, &bin_path)?;
        }

        // On Windows, copy to a directory that might be in PATH
        #[cfg(windows)]
        {
            // Try to add to PATH or inform user
            self.update_windows_path()?;
        }

        self.log_success(&format!("Kopi binary installed to {}", dest_path.display()));
        Ok(())
    }

    #[cfg(windows)]
    fn update_windows_path(&self) -> Result<(), InstallerError> {
        self.log_info("Note: You may need to add the installation directory to your PATH");
        self.log_info(&format!("Installation directory: {}", self.install_dir.display()));
        Ok(())
    }

    fn create_uninstaller(&self) -> Result<(), InstallerError> {
        self.log_info("Creating uninstaller...");
        
        let home_dir = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        
        let uninstall_script = if cfg!(windows) {
            format!("@echo off\necho Uninstalling Kopi Language...\ndel /f /q \"{}\\kopi.exe\" 2>nul\nrmdir /s /q \"{}\" 2>nul\necho Kopi has been uninstalled successfully\npause", 
                self.install_dir.display(), self.install_dir.display())
        } else {
            format!("#!/bin/bash\necho \"Uninstalling Kopi Language...\"\nrm -f \"{}/kopi\"\nrm -f \"{}/.local/bin/kopi\"\nrm -rf \"{}\"\necho \"Kopi has been uninstalled successfully\"", 
                self.install_dir.display(), home_dir, self.install_dir.display())
        };

        let uninstall_path = if cfg!(windows) {
            self.install_dir.join("uninstall.bat")
        } else {
            self.install_dir.join("uninstall.sh")
        };

        fs::write(&uninstall_path, uninstall_script)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&uninstall_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&uninstall_path, perms)?;
        }

        self.log_success(&format!("Uninstaller created at {}", uninstall_path.display()));
        Ok(())
    }

    fn cleanup(&self) -> Result<(), InstallerError> {
        if self.temp_dir.exists() {
            self.log_info("Cleaning up temporary files...");
            fs::remove_dir_all(&self.temp_dir)?;
        }
        Ok(())
    }

    fn verify_installation(&self) -> Result<(), InstallerError> {
        self.log_info("Verifying installation...");
        
        let binary_name = if cfg!(windows) { "kopi.exe" } else { "kopi" };
        let binary_path = self.install_dir.join(binary_name);
        
        if binary_path.exists() {
            self.log_success("Kopi installed successfully!");
            println!();
            
            if self.command_exists("kopi") {
                self.log_info("Kopi is ready to use:");
                println!("  \x1b[32mkopi --help\x1b[0m");
                println!("  \x1b[32mkopi your_script.kopi\x1b[0m");
            } else {
                self.log_warning("Kopi installed but may not be in PATH yet");
                println!("  \x1b[32m{} --help\x1b[0m", binary_path.display());
                println!("  \x1b[32m{} your_script.kopi\x1b[0m", binary_path.display());
            }
            
            println!();
            self.log_info("To uninstall Kopi later, run the uninstaller:");
            let uninstall_name = if cfg!(windows) { "uninstall.bat" } else { "uninstall.sh" };
            println!("  \x1b[32m{}\x1b[0m", self.install_dir.join(uninstall_name).display());
            
            Ok(())
        } else {
            Err(InstallerError::PathError("Installation verification failed".to_string()))
        }
    }

    fn uninstall(&self) -> Result<(), InstallerError> {
        self.log_info("Uninstalling Kopi...");
        
        let binary_name = if cfg!(windows) { "kopi.exe" } else { "kopi" };
        let binary_path = self.install_dir.join(binary_name);
        
        if binary_path.exists() {
            fs::remove_file(&binary_path)?;
        }

        #[cfg(unix)]
        {
            let bin_path = self.bin_dir.join("kopi");
            if bin_path.exists() {
                fs::remove_file(&bin_path)?;
            }
        }

        if self.install_dir.exists() {
            fs::remove_dir_all(&self.install_dir)?;
        }

        self.log_success("Kopi has been uninstalled successfully");
        Ok(())
    }

    fn install(&self) -> Result<(), InstallerError> {
        self.print_banner();

        // Check if already installed
        let binary_name = if cfg!(windows) { "kopi.exe" } else { "kopi" };
        let binary_path = self.install_dir.join(binary_name);
        
        if binary_path.exists() {
            self.log_warning("Kopi appears to already be installed");
            print!("Do you want to reinstall? (y/N): ");
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            
            if !input.trim().to_lowercase().starts_with('y') {
                self.log_info("Installation cancelled");
                return Ok(());
            }
        }

        self.log_info("Starting Kopi installation...");

        self.check_dependencies()?;
        self.create_directories()?;
        self.download_and_build()?;
        self.install_binary()?;
        self.create_uninstaller()?;
        self.verify_installation()?;

        println!();
        self.log_success("🎉 Kopi installation completed successfully!");
        println!();
        println!("\x1b[34mHappy coding with Kopi! ☕\x1b[0m");

        Ok(())
    }
}

fn show_help() {
    println!("Kipper - The Kopi Language Installer");
    println!();
    println!("USAGE:");
    println!("    {} [OPTIONS]", INSTALLER_NAME);
    println!();
    println!("OPTIONS:");
    println!("    -h, --help        Show this help message");
    println!("    -u, --uninstall   Uninstall Kopi");
    println!("    -v, --version     Show version information");
    println!();
    println!("EXAMPLES:");
    println!("    {}              Install Kopi", INSTALLER_NAME);
    println!("    {} --uninstall  Uninstall Kopi", INSTALLER_NAME);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    let installer = match Installer::new() {
        Ok(installer) => installer,
        Err(e) => {
            eprintln!("Failed to initialize installer: {:?}", e);
            std::process::exit(1);
        }
    };

    let result = match args.get(1).map(String::as_str) {
        Some("-h") | Some("--help") => {
            show_help();
            Ok(())
        }
        Some("-u") | Some("--uninstall") => {
            installer.uninstall()
        }
        Some("-v") | Some("--version") => {
            println!("Kipper v0.1.0 - The Kopi Language Installer");
            Ok(())
        }
        None => {
            installer.install()
        }
        Some(arg) => {
            eprintln!("Unknown option: {}", arg);
            show_help();
            std::process::exit(1);
        }
    };

    let _ = installer.cleanup();

    if let Err(e) = result {
        installer.log_error(&format!("{:?}", e));
        std::process::exit(1);
    }
}
