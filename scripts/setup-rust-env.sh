#!/bin/bash
# Setup environment for running Rust tests with PyO3
#
# Source this script before running cargo test:
#   source scripts/setup-rust-env.sh
#   cargo test
#
# Or add the export line to your shell profile (~/.zshrc, ~/.bashrc) for permanent setup.

# Get Python library directory
PYTHON_LIBDIR=$(python3 -c "import sysconfig; print(sysconfig.get_config_var('LIBDIR'))" 2>/dev/null)

if [ -z "$PYTHON_LIBDIR" ]; then
    echo "Error: Could not determine Python library directory"
    echo "Make sure python3 is available and working"
    return 1 2>/dev/null || exit 1
fi

# Detect OS and set appropriate library path variable
case "$(uname -s)" in
    Darwin*)
        # macOS
        export DYLD_LIBRARY_PATH="${PYTHON_LIBDIR}${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
        echo "✓ Set DYLD_LIBRARY_PATH to include: ${PYTHON_LIBDIR}"
        ;;
    Linux*)
        # Linux
        export LD_LIBRARY_PATH="${PYTHON_LIBDIR}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
        echo "✓ Set LD_LIBRARY_PATH to include: ${PYTHON_LIBDIR}"
        ;;
    *)
        echo "Warning: Unknown OS type: $(uname -s)"
        echo "You may need to manually set library paths for PyO3"
        ;;
esac

# Verify setup
echo "✓ Using Python: $(which python3)"
echo "✓ Python version: $(python3 --version 2>&1)"
echo ""
echo "You can now run: cargo test"
