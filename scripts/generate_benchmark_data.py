#!/usr/bin/env python3
"""Generate synthetic benchmark datasets for cloud IO benchmarks.

Creates synthetic imagery files by invoking ``generate_synthetic_image.py``
with predefined configurations, then appends the generated dataset entries
to ``benchmark_datasets.yaml``.

Usage::

    python scripts/generate_benchmark_data.py
    python scripts/generate_benchmark_data.py --output-dir data/integration/synthetic
    python scripts/generate_benchmark_data.py --config-file data/benchmark/benchmark_datasets.yaml
"""

from __future__ import annotations

import argparse
import logging
import subprocess
import sys
from pathlib import Path
from typing import Any

import yaml

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Dataset configurations
# ---------------------------------------------------------------------------

#: Default dataset configurations for synthetic benchmark imagery.
#: Each entry maps to a single invocation of ``generate_synthetic_image.py``.
DATASET_CONFIGS: list[dict[str, Any]] = [
    {
        "filename": "synth_small_nc.ntf",
        "label": "Synth Small NC",
        "args": [
            "--format", "nitf",
            "--width", "1024",
            "--height", "1024",
            "--bands", "1",
            "--tile-width", "256",
            "--tile-height", "256",
            "--compression", "none",
        ],
    },
    {
        "filename": "synth_medium_c3.ntf",
        "label": "Synth Medium C3",
        "args": [
            "--format", "nitf",
            "--width", "2048",
            "--height", "2048",
            "--bands", "3",
            "--tile-width", "256",
            "--tile-height", "256",
            "--compression", "jpeg",
        ],
    },
    {
        "filename": "synth_medium_c8.ntf",
        "label": "Synth Medium C8",
        "args": [
            "--format", "nitf",
            "--width", "2048",
            "--height", "2048",
            "--bands", "1",
            "--tile-width", "512",
            "--tile-height", "512",
            "--compression", "j2k",
        ],
    },
    {
        "filename": "synth_small_tiff.tif",
        "label": "Synth Small TIFF",
        "args": [
            "--format", "tiff",
            "--width", "1024",
            "--height", "1024",
            "--bands", "1",
            "--tile-width", "256",
            "--tile-height", "256",
            "--compression", "none",
        ],
    },
    {
        "filename": "synth_large_nc.ntf",
        "label": "Synth Large NC",
        "args": [
            "--format", "nitf",
            "--width", "8192",
            "--height", "8192",
            "--bands", "1",
            "--tile-width", "1024",
            "--tile-height", "1024",
            "--compression", "none",
        ],
    },
]

# ---------------------------------------------------------------------------
# Path to the companion generator script
# ---------------------------------------------------------------------------

_SCRIPT_DIR = Path(__file__).resolve().parent
_GENERATE_IMAGE_SCRIPT = _SCRIPT_DIR / "generate_synthetic_image.py"

# ---------------------------------------------------------------------------
# Default paths (relative to project root)
# ---------------------------------------------------------------------------

_PROJECT_ROOT = _SCRIPT_DIR.parent
_DEFAULT_OUTPUT_DIR = _PROJECT_ROOT / "data" / "integration" / "synthetic"
_DEFAULT_CONFIG_FILE = _PROJECT_ROOT / "data" / "benchmark" / "benchmark_datasets.yaml"


def _generate_single_dataset(
    config: dict[str, Any],
    output_dir: Path,
) -> Path | None:
    """Invoke ``generate_synthetic_image.py`` for a single dataset config.

    Returns the output path on success, or ``None`` on failure.
    """
    output_path = output_dir / config["filename"]

    # Idempotent: skip if file already exists
    if output_path.exists():
        logger.info("Skipping %s — file already exists", output_path)
        return output_path

    cmd = [
        sys.executable,
        str(_GENERATE_IMAGE_SCRIPT),
        str(output_path),
        *config["args"],
    ]

    logger.info("Generating %s ...", config["label"])
    logger.debug("Command: %s", " ".join(cmd))

    try:
        subprocess.run(cmd, check=True, capture_output=True, text=True)
    except subprocess.CalledProcessError as exc:
        logger.warning(
            "Failed to generate %s: %s\nstdout: %s\nstderr: %s",
            config["label"],
            exc,
            exc.stdout,
            exc.stderr,
        )
        return None

    logger.info("Created %s", output_path)
    return output_path


def _load_existing_labels(config_file: Path) -> set[str]:
    """Return the set of dataset labels already present in the YAML config."""
    if not config_file.exists():
        return set()

    with open(config_file, "r") as fh:
        data = yaml.safe_load(fh) or {}

    return {entry["label"] for entry in data.get("datasets", []) if "label" in entry}


def _append_datasets_to_yaml(
    config_file: Path,
    new_entries: list[dict[str, str]],
) -> None:
    """Append new dataset entries to ``benchmark_datasets.yaml``.

    Only entries whose label is not already present are appended.
    """
    if not new_entries:
        return

    existing_labels = _load_existing_labels(config_file)

    entries_to_add = [
        entry for entry in new_entries if entry["label"] not in existing_labels
    ]

    if not entries_to_add:
        logger.info("All generated datasets already present in %s", config_file)
        return

    # Load existing data (or create skeleton)
    if config_file.exists():
        with open(config_file, "r") as fh:
            data = yaml.safe_load(fh) or {}
    else:
        data = {}

    if "datasets" not in data:
        data["datasets"] = []

    data["datasets"].extend(entries_to_add)

    with open(config_file, "w") as fh:
        yaml.dump(data, fh, default_flow_style=False, sort_keys=False)

    logger.info(
        "Appended %d dataset(s) to %s",
        len(entries_to_add),
        config_file,
    )


def generate_benchmark_data(
    output_dir: Path,
    config_file: Path,
) -> None:
    """Generate synthetic benchmark datasets and update the YAML config.

    For each dataset configuration in :data:`DATASET_CONFIGS`:

    1. Invoke ``generate_synthetic_image.py`` to create the file (skipped if
       the output file already exists).
    2. Collect successfully generated entries.
    3. Append new entries to *config_file* (entries already present by label
       are skipped).

    Parameters
    ----------
    output_dir:
        Directory where synthetic files are written.
    config_file:
        Path to ``benchmark_datasets.yaml``.
    """
    output_dir.mkdir(parents=True, exist_ok=True)

    new_entries: list[dict[str, str]] = []

    for dataset_config in DATASET_CONFIGS:
        result = _generate_single_dataset(dataset_config, output_dir)
        if result is not None:
            # Store path relative to the benchmark data root so it resolves
            # correctly via OSML_IO_BENCHMARK_DATA or data/benchmark/.
            relative_path = str(
                Path("synthetic") / dataset_config["filename"]
            )
            new_entries.append(
                {
                    "path": relative_path,
                    "label": dataset_config["label"],
                }
            )

    _append_datasets_to_yaml(config_file, new_entries)


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def parse_args(args: list[str] | None = None) -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(
        description="Generate synthetic benchmark datasets for cloud IO benchmarks.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=_DEFAULT_OUTPUT_DIR,
        help=(
            "Directory for generated synthetic files "
            f"(default: {_DEFAULT_OUTPUT_DIR})"
        ),
    )
    parser.add_argument(
        "--config-file",
        type=Path,
        default=_DEFAULT_CONFIG_FILE,
        help=(
            "Path to benchmark_datasets.yaml "
            f"(default: {_DEFAULT_CONFIG_FILE})"
        ),
    )
    return parser.parse_args(args)


def main() -> int:
    """Entry point."""
    logging.basicConfig(
        level=logging.INFO,
        format="%(levelname)s: %(message)s",
    )

    ns = parse_args()
    generate_benchmark_data(output_dir=ns.output_dir, config_file=ns.config_file)
    return 0


if __name__ == "__main__":
    sys.exit(main())
