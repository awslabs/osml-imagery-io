#!/bin/bash
# Verify that a release wheel's extension module has no unresolved references to
# the C libraries that are supposed to be statically linked into it.
#
# A `-shared` / `-dylib` link does not fail on undefined symbols, so a broken
# link order produces a wheel that builds and installs cleanly and then fails at
# `import aws.osml.io` with e.g. `undefined symbol: ZSTD_compressStream`. This
# check turns that into a build-time failure.
#
# Usage:
#   scripts/check-wheel-symbols.sh                 # check target/wheels/*.whl
#   scripts/check-wheel-symbols.sh path/to/*.whl   # check specific wheels

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [[ $# -gt 0 ]]; then
    WHEELS=("$@")
else
    shopt -s nullglob
    WHEELS=("$PROJECT_ROOT"/target/wheels/*.whl)
    shopt -u nullglob
fi

if [[ ${#WHEELS[@]} -eq 0 ]]; then
    echo "Error: no wheels found. Run 'scripts/build-wheel.sh' first." >&2
    exit 1
fi

# Symbol prefixes owned by the statically-linked C libraries. Any of these left
# undefined in the extension module means its archive was dropped at link time.
PREFIXES='ZSTD_|lerc_|Lerc_|libdeflate_|TIFF|_TIFF|opj_|_opj_|jpeg_|_jpeg_|tj[A-Z]'

STATUS=0

for whl in "${WHEELS[@]}"; do
    workdir="$(mktemp -d)"
    trap 'rm -rf "$workdir"' EXIT
    unzip -q "$whl" -d "$workdir"

    so="$(find "$workdir" -name '_io*.so' -o -name '_io*.pyd' -o -name '_io*.dylib' | head -1)"
    if [[ -z "$so" ]]; then
        echo "Error: no extension module found in $(basename "$whl")" >&2
        STATUS=1
        rm -rf "$workdir"
        trap - EXIT
        continue
    fi

    # Weak undefined symbols are excluded: libzstd declares optional tracing
    # hooks (ZSTD_trace_*) that are meant to stay unresolved.
    if command -v readelf &> /dev/null && readelf -h "$so" &> /dev/null; then
        undefined="$(readelf --dyn-syms --wide "$so" | awk '$5 == "GLOBAL" && $7 == "UND" { print $8 }')"
    else
        # macOS: no readelf. `nm -u` marks weak undefined symbols with 'w'/'v'
        # (or the words "weak external" for Mach-O).
        undefined="$(nm -u "$so" 2>/dev/null | grep -vE '(^|[[:space:]])[wv][[:space:]]|weak' || true)"
    fi

    missing="$(echo "$undefined" | grep -oE "\b(${PREFIXES})[A-Za-z0-9_]*" | sort -u || true)"

    if [[ -n "$missing" ]]; then
        echo "FAIL: $(basename "$whl") has unresolved codec symbols:"
        echo "$missing" | sed 's/^/  /'
        echo "  → a static archive was dropped at link time; check link order in build.rs"
        STATUS=1
    else
        echo "OK: $(basename "$whl")"
    fi

    rm -rf "$workdir"
    trap - EXIT
done

exit $STATUS
