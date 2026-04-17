#!/bin/bash
# Setup development environment for osml-imagery-io
#
# This script configures the shell environment for both Rust and Python development.
# Source this script in your terminal before running tests:
#
#   source scripts/setup-dev-env.sh
#
# For permanent setup, add the following to your shell profile (~/.zshrc or ~/.bashrc):
#
#   source /path/to/osml-imagery-io/scripts/setup-dev-env.sh
#
# Prerequisites:
#   - Python 3.9+ with a virtual environment activated (venv or conda)
#   - Rust toolchain installed

# Only set -e when run as a script, not when sourced into an interactive shell
if [[ ! -o interactive ]]; then
  set -e
fi

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo_success() { echo -e "${GREEN}✓${NC} $1"; }
echo_warning() { echo -e "${YELLOW}⚠${NC} $1"; }
echo_error() { echo -e "${RED}✗${NC} $1"; }

# Check for Python
if ! command -v python3 &> /dev/null; then
    echo_error "python3 not found. Please install Python 3.9+ first."
    return 1 2>/dev/null || exit 1
fi

# Check Python version
PYTHON_VERSION=$(python3 -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')")
PYTHON_MAJOR=$(echo $PYTHON_VERSION | cut -d. -f1)
PYTHON_MINOR=$(echo $PYTHON_VERSION | cut -d. -f2)

if [ "$PYTHON_MAJOR" -lt 3 ] || ([ "$PYTHON_MAJOR" -eq 3 ] && [ "$PYTHON_MINOR" -lt 9 ]); then
    echo_error "Python 3.9+ required, found $PYTHON_VERSION"
    return 1 2>/dev/null || exit 1
fi

echo_success "Python $PYTHON_VERSION detected: $(which python3)"

# Check if we're in a virtual environment
if [ -z "$VIRTUAL_ENV" ] && [ -z "$CONDA_PREFIX" ]; then
    echo_warning "No virtual environment detected."
    echo "  Consider creating one with:"
    echo "    python3 -m venv .venv && source .venv/bin/activate"
    echo "  Or with conda:"
    echo "    conda create -n osml-io python=3.11 && conda activate osml-io"
    echo ""
elif [ -n "$VIRTUAL_ENV" ] && [ -n "$CONDA_PREFIX" ]; then
    # Both venv and conda are active - this can cause issues
    echo_warning "Both venv and conda are active. This may cause conflicts."
    echo "  VIRTUAL_ENV: $VIRTUAL_ENV"
    echo "  CONDA_PREFIX: $CONDA_PREFIX"
    echo "  Consider using one or the other:"
    echo "    - Deactivate conda first: conda deactivate"
    echo "    - Or use a conda env instead: conda create -n osml-io python=3.11"
    echo ""
else
    if [ -n "$VIRTUAL_ENV" ]; then
        echo_success "Virtual environment active: $VIRTUAL_ENV"
    elif [ -n "$CONDA_PREFIX" ]; then
        echo_success "Conda environment active: $CONDA_PREFIX"
    fi
fi

# Setup Python library path for PyO3 (needed for cargo test)
PYTHON_LIBDIR=$(python3 -c "import sysconfig; print(sysconfig.get_config_var('LIBDIR'))" 2>/dev/null)

if [ -z "$PYTHON_LIBDIR" ]; then
    echo_error "Could not determine Python library directory"
    return 1 2>/dev/null || exit 1
fi

# Detect OS and set appropriate library path variable
case "$(uname -s)" in
    Darwin*)
        # macOS
        if [[ ":$DYLD_LIBRARY_PATH:" != *":$PYTHON_LIBDIR:"* ]]; then
            export DYLD_LIBRARY_PATH="${PYTHON_LIBDIR}${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
        fi
        echo_success "DYLD_LIBRARY_PATH includes: ${PYTHON_LIBDIR}"
        
        # Add conda lib directory for native libraries (e.g., OpenJPEG)
        if [ -n "$CONDA_PREFIX" ] && [ -d "$CONDA_PREFIX/lib" ]; then
            if [[ ":$DYLD_LIBRARY_PATH:" != *":$CONDA_PREFIX/lib:"* ]]; then
                export DYLD_LIBRARY_PATH="${CONDA_PREFIX}/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
            fi
            echo_success "DYLD_LIBRARY_PATH includes: ${CONDA_PREFIX}/lib"
        fi
        ;;
    Linux*)
        # Linux
        if [[ ":$LD_LIBRARY_PATH:" != *":$PYTHON_LIBDIR:"* ]]; then
            export LD_LIBRARY_PATH="${PYTHON_LIBDIR}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
        fi
        echo_success "LD_LIBRARY_PATH includes: ${PYTHON_LIBDIR}"
        
        # Add conda lib directory for native libraries (e.g., OpenJPEG)
        if [ -n "$CONDA_PREFIX" ] && [ -d "$CONDA_PREFIX/lib" ]; then
            if [[ ":$LD_LIBRARY_PATH:" != *":$CONDA_PREFIX/lib:"* ]]; then
                export LD_LIBRARY_PATH="${CONDA_PREFIX}/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
            fi
            echo_success "LD_LIBRARY_PATH includes: ${CONDA_PREFIX}/lib"
        fi
        ;;
    *)
        echo_warning "Unknown OS: $(uname -s). You may need to set library paths manually."
        ;;
esac

# Check for Rust
if command -v rustc &> /dev/null; then
    RUST_VERSION=$(rustc --version | cut -d' ' -f2)
    echo_success "Rust $RUST_VERSION detected"
else
    echo_warning "Rust not found. Install from https://rustup.rs/"
fi

# Check for maturin
if command -v maturin &> /dev/null; then
    MATURIN_VERSION=$(maturin --version | cut -d' ' -f2)
    echo_success "Maturin $MATURIN_VERSION detected"
else
    echo_warning "Maturin not found. Install with: pip install maturin"
fi

# =============================================================================
# Native Library Version Checks
# =============================================================================
# These checks verify that the dynamically linked C libraries in the conda
# environment match the versions and ABI expected by the Rust FFI bindings in
# src/*/sys.rs. A mismatch can cause silent memory corruption.
#
# See docs/design/native-library-ffi.md for background.
# =============================================================================

FFI_CHECK_FAILED=0

# --- Expected versions (must match release.yml and sys.rs) ---
# JPEG_LIB_VERSION is the emulated IJG ABI version, NOT the libjpeg-turbo
# release version. It is controlled by the -DWITH_JPEG8 build flag.
# Our release workflow builds with -DWITH_JPEG8=1, so the expected ABI is 80.
EXPECTED_JPEG_LIB_VERSION=80
EXPECTED_OPENJPEG_MAJOR=2
EXPECTED_OPENJPEG_MINOR=5

check_native_libs() {
    local include_dir=""

    if [ -n "$CONDA_PREFIX" ] && [ -d "$CONDA_PREFIX/include" ]; then
        include_dir="$CONDA_PREFIX/include"
    elif [ -d "/usr/local/include" ]; then
        include_dir="/usr/local/include"
    elif [ -d "/opt/homebrew/include" ]; then
        include_dir="/opt/homebrew/include"
    fi

    if [ -z "$include_dir" ]; then
        echo_warning "Could not find native library headers. Skipping FFI checks."
        return
    fi

    # --- libjpeg-turbo: check JPEG_LIB_VERSION (ABI version) ---
    local jconfig="$include_dir/jconfig.h"
    if [ -f "$jconfig" ]; then
        local jpeg_lib_ver
        jpeg_lib_ver=$(grep '#define JPEG_LIB_VERSION ' "$jconfig" 2>/dev/null | awk '{print $3}')
        local jpeg_turbo_ver
        jpeg_turbo_ver=$(grep '#define LIBJPEG_TURBO_VERSION ' "$jconfig" 2>/dev/null | awk '{print $3}')

        if [ -n "$jpeg_lib_ver" ]; then
            if [ "$jpeg_lib_ver" -eq "$EXPECTED_JPEG_LIB_VERSION" ]; then
                echo_success "libjpeg-turbo ${jpeg_turbo_ver:-unknown} (JPEG_LIB_VERSION=$jpeg_lib_ver)"
            else
                echo_error "libjpeg-turbo JPEG_LIB_VERSION=$jpeg_lib_ver, expected $EXPECTED_JPEG_LIB_VERSION"
                echo "  The Rust FFI structs in src/jpeg/sys.rs are written for ABI version $EXPECTED_JPEG_LIB_VERSION."
                echo "  The installed library uses ABI version $jpeg_lib_ver (struct layouts differ)."
                echo "  This will cause incorrect field offsets when using the libjpeg API."
                echo "  See docs/design/native-library-ffi.md for details."
                FFI_CHECK_FAILED=1
            fi
        fi
    else
        echo_warning "jconfig.h not found — cannot verify libjpeg-turbo ABI version"
    fi

    # --- OpenJPEG: check major.minor version ---
    local opj_config="$include_dir/openjpeg-2.5/opj_config.h"
    # Try alternate paths
    [ ! -f "$opj_config" ] && opj_config="$include_dir/openjpeg-2.4/opj_config.h"
    [ ! -f "$opj_config" ] && opj_config="$include_dir/openjpeg.h"

    if [ -f "$opj_config" ]; then
        local opj_major opj_minor opj_build
        opj_major=$(grep '#define OPJ_VERSION_MAJOR' "$opj_config" 2>/dev/null | awk '{print $3}')
        opj_minor=$(grep '#define OPJ_VERSION_MINOR' "$opj_config" 2>/dev/null | awk '{print $3}')
        opj_build=$(grep '#define OPJ_VERSION_BUILD' "$opj_config" 2>/dev/null | awk '{print $3}')

        if [ -n "$opj_major" ] && [ -n "$opj_minor" ]; then
            if [ "$opj_major" -eq "$EXPECTED_OPENJPEG_MAJOR" ] && [ "$opj_minor" -eq "$EXPECTED_OPENJPEG_MINOR" ]; then
                echo_success "OpenJPEG ${opj_major}.${opj_minor}.${opj_build:-0}"
            else
                echo_warning "OpenJPEG ${opj_major}.${opj_minor}.${opj_build:-0} (expected ${EXPECTED_OPENJPEG_MAJOR}.${EXPECTED_OPENJPEG_MINOR}.x)"
                echo "  A different major.minor version may have different struct layouts."
                echo "  Run the FFI verification procedure in docs/design/native-library-ffi.md."
            fi
        fi
    else
        echo_warning "OpenJPEG headers not found — cannot verify version"
    fi

    # --- libtiff: report version (low risk, opaque handles) ---
    local tiffvers="$include_dir/tiffvers.h"
    if [ -f "$tiffvers" ]; then
        local tiff_ver
        tiff_ver=$(grep '#define TIFFLIB_VERSION_STR_MAJ_MIN_MIC' "$tiffvers" 2>/dev/null | sed 's/.*"\(.*\)".*/\1/')
        if [ -n "$tiff_ver" ]; then
            echo_success "libtiff $tiff_ver"
        fi
    else
        echo_warning "libtiff headers not found — cannot verify version"
    fi
}

check_native_libs

if [ "$FFI_CHECK_FAILED" -eq 1 ]; then
    echo ""
    echo_warning "FFI version mismatch detected. The Rust FFI struct definitions may not"
    echo "  match the installed C library headers. This can cause silent memory"
    echo "  corruption when using the libjpeg API (not TurboJPEG)."
    echo "  See docs/design/native-library-ffi.md for the verification procedure."
fi

echo ""
echo "Environment ready. Available commands:"
echo "  maturin develop    - Build and install Python extension"
echo "  pytest             - Run Python tests"
echo "  cargo test         - Run Rust tests"
echo "  cargo clippy       - Lint Rust code"
echo "  ruff check .       - Lint Python code"
