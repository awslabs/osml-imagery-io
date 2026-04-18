//! Build script for osml-imagery-io.
//!
//! This script handles finding and linking to native libraries like OpenJPEG.

fn main() {
    // When the static feature is enabled, use the static linking path and skip dynamic discovery.
    // This is used in CI release builds where C libraries are compiled from source.
    #[cfg(feature = "static")]
    {
        configure_static_linking();
        return;
    }

    // Dynamic linking path for local development
    #[cfg(feature = "openjpeg")]
    {
        configure_openjpeg();
    }

    #[cfg(feature = "libtiff")]
    {
        configure_libtiff();
    }
}

/// Configure static linking of native C libraries for release wheel builds.
///
/// Reads `DEP_OPENJP2_ROOT`, `DEP_JPEG_ROOT`, and `DEP_TIFF_ROOT` environment variables
/// to locate pre-compiled static archives (.a files), then emits the appropriate
/// `cargo:rustc-link-lib=static=...` and `cargo:rustc-link-search=native=...` directives.
///
/// Panics with a descriptive error if any required env var is unset or a `.a` file is missing.
#[cfg(feature = "static")]
fn configure_static_linking() {
    use std::path::Path;

    let openjp2_root = required_env("DEP_OPENJP2_ROOT", "OpenJPEG");
    let jpeg_root = required_env("DEP_JPEG_ROOT", "libjpeg-turbo");
    let tiff_root = required_env("DEP_TIFF_ROOT", "libtiff");
    let deflate_root = required_env("DEP_DEFLATE_ROOT", "libdeflate");
    let zstd_root = required_env("DEP_ZSTD_ROOT", "zstd");
    let lerc_root = required_env("DEP_LERC_ROOT", "LERC");

    // Search paths — include both lib/ and lib64/ for all libraries
    // since cmake may install to either depending on the platform.
    for root in [
        &openjp2_root,
        &jpeg_root,
        &tiff_root,
        &deflate_root,
        &zstd_root,
        &lerc_root,
    ] {
        println!("cargo:rustc-link-search=native={}/lib", root);
        println!("cargo:rustc-link-search=native={}/lib64", root);
    }
    // Optional: cross-compiled zlib and xz for aarch64 builds
    if let Ok(zlib_root) = std::env::var("DEP_ZLIB_ROOT") {
        println!("cargo:rustc-link-search=native={}/lib", zlib_root);
        println!("cargo:rustc-link-search=native={}/lib64", zlib_root);
    }
    if let Ok(xz_root) = std::env::var("DEP_XZ_ROOT") {
        println!("cargo:rustc-link-search=native={}/lib", xz_root);
        println!("cargo:rustc-link-search=native={}/lib64", xz_root);
    }

    // Verify static archives exist before linking
    require_static_lib(&openjp2_root, "openjp2");
    require_static_lib(&jpeg_root, "turbojpeg");
    require_static_lib(&jpeg_root, "jpeg");
    require_static_lib(&tiff_root, "tiff");
    require_static_lib(&deflate_root, "deflate");
    require_static_lib(&zstd_root, "zstd");
    require_static_lib(&lerc_root, "Lerc");

    // Static link directives (these override the #[link] attributes in sys modules)
    // We must force-load libtiff to prevent the linker from stripping codec init
    // functions (TIFFInitJPEG, TIFFInitZSTD, etc.) that are only called indirectly
    // via libtiff's codec dispatch table. OpenJPEG also needs force-load because
    // it uses function pointers and thread-local storage that get stripped otherwise.
    let tiff_lib = find_static_lib(&tiff_root, "tiff");
    let openjp2_lib = find_static_lib(&openjp2_root, "openjp2");
    #[cfg(target_os = "macos")]
    {
        // force_load libtiff to keep codec init functions (TIFFInitJPEG, etc.)
        // that are only called indirectly via libtiff's codec dispatch table.
        // force_load OpenJPEG to keep encode functions that are only called
        // through FFI declarations and would otherwise be stripped by the linker.
        println!("cargo:rustc-link-arg=-Wl,-force_load,{}", tiff_lib);
        println!("cargo:rustc-link-arg=-Wl,-force_load,{}", openjp2_lib);
    }
    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-arg=-Wl,--whole-archive");
        println!("cargo:rustc-link-arg={}", tiff_lib);
        println!("cargo:rustc-link-arg={}", openjp2_lib);
        println!("cargo:rustc-link-arg=-Wl,--no-whole-archive");
    }
    println!("cargo:rustc-link-lib=static=Lerc");
    println!("cargo:rustc-link-lib=static=zstd");
    println!("cargo:rustc-link-lib=static=deflate");
    println!("cargo:rustc-link-lib=static=turbojpeg");
    println!("cargo:rustc-link-lib=static=jpeg");

    // Platform-specific transitive dependencies required by libtiff and LERC
    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-lib=z");
        println!("cargo:rustc-link-lib=lzma");
        println!("cargo:rustc-link-lib=stdc++");
    }
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=z");
        println!("cargo:rustc-link-lib=c++");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    }
}

/// Read a required environment variable, panicking with a helpful message if unset.
#[cfg(feature = "static")]
fn required_env(var: &str, lib_name: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| {
        panic!(
            "static feature enabled but {} not set. Set it to the {} install prefix.",
            var, lib_name
        )
    })
}

/// Verify that a static library archive exists under `{root}/lib/lib{name}.a`
/// or `{root}/lib64/lib{name}.a`. Panics if neither is found.
#[cfg(feature = "static")]
fn require_static_lib(root: &str, name: &str) {
    use std::path::Path;

    let lib_path = Path::new(root).join("lib").join(format!("lib{}.a", name));
    let lib64_path = Path::new(root).join("lib64").join(format!("lib{}.a", name));

    if !lib_path.exists() && !lib64_path.exists() {
        panic!(
            "static feature enabled but lib{}.a not found in {}/lib (or {}/lib64)",
            name, root, root
        );
    }
}

/// Find the full path to a static library archive under `{root}/lib/lib{name}.a`
/// or `{root}/lib64/lib{name}.a`. Panics if neither is found.
#[cfg(feature = "static")]
fn find_static_lib(root: &str, name: &str) -> String {
    use std::path::Path;

    let lib_path = Path::new(root).join("lib").join(format!("lib{}.a", name));
    if lib_path.exists() {
        return lib_path.to_string_lossy().into_owned();
    }
    let lib64_path = Path::new(root).join("lib64").join(format!("lib{}.a", name));
    if lib64_path.exists() {
        return lib64_path.to_string_lossy().into_owned();
    }
    panic!(
        "static feature enabled but lib{}.a not found in {}/lib (or {}/lib64)",
        name, root, root
    );
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
        "/opt/homebrew/lib",           // Apple Silicon Homebrew
        "/usr/local/opt/openjpeg/lib", // Intel Homebrew
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
