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

echo ""
echo "Environment ready. Available commands:"
echo "  maturin develop    - Build and install Python extension"
echo "  pytest             - Run Python tests"
echo "  cargo test         - Run Rust tests"
echo "  cargo clippy       - Lint Rust code"
echo "  ruff check .       - Lint Python code"
