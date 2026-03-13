//! Build script for osml-imagery-io.
//!
//! This script handles finding and linking to native libraries like OpenJPEG.

fn main() {
    // Only configure OpenJPEG linking when the feature is enabled
    #[cfg(feature = "openjpeg")]
    {
        configure_openjpeg();
    }

    // Only configure libtiff linking when the feature is enabled
    #[cfg(feature = "libtiff")]
    {
        configure_libtiff();
    }
}

#[cfg(feature = "openjpeg")]
fn configure_openjpeg() {
    // Try pkg-config first (works on most Unix systems)
    if try_pkg_config() {
        return;
    }

    // Fall back to system library search
    if try_system_library() {
        return;
    }

    // If we get here, we couldn't find OpenJPEG
    eprintln!("Warning: Could not find libopenjp2. JPEG 2000 support may not work.");
    eprintln!("Install OpenJPEG:");
    eprintln!("  macOS:   brew install openjpeg");
    eprintln!("  Ubuntu:  apt-get install libopenjp2-7-dev");
    eprintln!("  Fedora:  dnf install openjpeg2-devel");
}

#[cfg(feature = "openjpeg")]
fn try_pkg_config() -> bool {
    // Check if pkg-config is available
    match std::process::Command::new("pkg-config")
        .args(["--exists", "libopenjp2"])
        .status()
    {
        Ok(status) if status.success() => {
            // Get the library flags from pkg-config
            let output = std::process::Command::new("pkg-config")
                .args(["--libs", "libopenjp2"])
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    let flags = String::from_utf8_lossy(&output.stdout);
                    
                    // Parse the flags and emit cargo directives
                    for flag in flags.split_whitespace() {
                        if let Some(lib) = flag.strip_prefix("-l") {
                            println!("cargo:rustc-link-lib={}", lib);
                        } else if let Some(path) = flag.strip_prefix("-L") {
                            println!("cargo:rustc-link-search=native={}", path);
                        }
                    }
                    
                    // Also get include path for documentation
                    let _ = std::process::Command::new("pkg-config")
                        .args(["--cflags", "libopenjp2"])
                        .output();
                    
                    return true;
                }
            }
        }
        _ => {}
    }
    false
}

#[cfg(feature = "openjpeg")]
fn try_system_library() -> bool {
    // Check conda environment first
    if let Ok(conda_prefix) = std::env::var("CONDA_PREFIX") {
        let conda_lib = format!("{}/lib", conda_prefix);
        let lib_path = std::path::Path::new(&conda_lib);
        if lib_path.exists() {
            let dylib = lib_path.join("libopenjp2.dylib");
            let so = lib_path.join("libopenjp2.so");
            let a = lib_path.join("libopenjp2.a");

            if dylib.exists() || so.exists() || a.exists() {
                println!("cargo:rustc-link-search=native={}", conda_lib);
                println!("cargo:rustc-link-lib=openjp2");
                return true;
            }
        }
    }

    // Common library search paths
    let search_paths = [
        "/usr/local/lib",
        "/usr/lib",
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/opt/homebrew/lib",  // Apple Silicon Homebrew
        "/usr/local/opt/openjpeg/lib",  // Intel Homebrew
    ];

    for path in &search_paths {
        let lib_path = std::path::Path::new(path);
        if lib_path.exists() {
            // Check for the library file
            let dylib = lib_path.join("libopenjp2.dylib");
            let so = lib_path.join("libopenjp2.so");
            let a = lib_path.join("libopenjp2.a");

            if dylib.exists() || so.exists() || a.exists() {
                println!("cargo:rustc-link-search=native={}", path);
                println!("cargo:rustc-link-lib=openjp2");
                return true;
            }
        }
    }

    // Last resort: just try to link and let the linker find it
    println!("cargo:rustc-link-lib=openjp2");
    true
}

#[cfg(feature = "libtiff")]
fn configure_libtiff() {
    // Try pkg-config first (works on most Unix systems)
    if try_pkg_config_libtiff() {
        return;
    }

    // Fall back to system library search
    if try_system_library_libtiff() {
        return;
    }

    // If we get here, we couldn't find libtiff
    eprintln!("Warning: Could not find libtiff. TIFF support may not work.");
    eprintln!("Install libtiff:");
    eprintln!("  macOS:   brew install libtiff");
    eprintln!("  Ubuntu:  apt-get install libtiff-dev");
    eprintln!("  Fedora:  dnf install libtiff-devel");
}

#[cfg(feature = "libtiff")]
fn try_pkg_config_libtiff() -> bool {
    match std::process::Command::new("pkg-config")
        .args(["--exists", "libtiff-4"])
        .status()
    {
        Ok(status) if status.success() => {
            let output = std::process::Command::new("pkg-config")
                .args(["--libs", "libtiff-4"])
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    let flags = String::from_utf8_lossy(&output.stdout);

                    for flag in flags.split_whitespace() {
                        if let Some(lib) = flag.strip_prefix("-l") {
                            println!("cargo:rustc-link-lib={}", lib);
                        } else if let Some(path) = flag.strip_prefix("-L") {
                            println!("cargo:rustc-link-search=native={}", path);
                        }
                    }

                    return true;
                }
            }
        }
        _ => {}
    }
    false
}

#[cfg(feature = "libtiff")]
fn try_system_library_libtiff() -> bool {
    // Check conda environment first
    if let Ok(conda_prefix) = std::env::var("CONDA_PREFIX") {
        let conda_lib = format!("{}/lib", conda_prefix);
        let lib_path = std::path::Path::new(&conda_lib);
        if lib_path.exists() {
            let dylib = lib_path.join("libtiff.dylib");
            let so = lib_path.join("libtiff.so");
            let a = lib_path.join("libtiff.a");

            if dylib.exists() || so.exists() || a.exists() {
                println!("cargo:rustc-link-search=native={}", conda_lib);
                println!("cargo:rustc-link-lib=tiff");
                return true;
            }
        }
    }

    // Common library search paths
    let search_paths = [
        "/usr/local/lib",
        "/usr/lib",
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/opt/homebrew/lib",
        "/usr/local/opt/libtiff/lib",
    ];

    for path in &search_paths {
        let lib_path = std::path::Path::new(path);
        if lib_path.exists() {
            let dylib = lib_path.join("libtiff.dylib");
            let so = lib_path.join("libtiff.so");
            let a = lib_path.join("libtiff.a");

            if dylib.exists() || so.exists() || a.exists() {
                println!("cargo:rustc-link-search=native={}", path);
                println!("cargo:rustc-link-lib=tiff");
                return true;
            }
        }
    }

    // Last resort: just try to link and let the linker find it
    println!("cargo:rustc-link-lib=tiff");
    true
}
