"""Unit tests for scripts/generate_benchmark_data.py."""

from __future__ import annotations

import subprocess
from pathlib import Path
from unittest.mock import patch

import pytest
import yaml

# Import the module under test
import importlib
import sys

# Add scripts/ to path so we can import the module
_SCRIPTS_DIR = Path(__file__).resolve().parent.parent / "scripts"
sys.path.insert(0, str(_SCRIPTS_DIR))
generate_benchmark_data = importlib.import_module("generate_benchmark_data")


class TestDatasetConfigs:
    """Verify the built-in dataset configurations are well-formed."""

    def test_all_configs_have_required_keys(self):
        for cfg in generate_benchmark_data.DATASET_CONFIGS:
            assert "filename" in cfg
            assert "label" in cfg
            assert "args" in cfg

    def test_expected_labels(self):
        labels = {cfg["label"] for cfg in generate_benchmark_data.DATASET_CONFIGS}
        assert labels == {
            "Synth Small NC",
            "Synth Medium C3",
            "Synth Medium C8",
            "Synth Small TIFF",
            "Synth Large NC",
        }


class TestIdempotency:
    """Verify that generation is skipped when the output file already exists."""

    def test_skips_existing_file(self, tmp_path):
        output_dir = tmp_path / "synthetic"
        output_dir.mkdir()

        config = generate_benchmark_data.DATASET_CONFIGS[0]
        existing_file = output_dir / config["filename"]
        existing_file.write_text("already here")

        with patch("subprocess.run") as mock_run:
            result = generate_benchmark_data._generate_single_dataset(config, output_dir)

        # subprocess should NOT have been called
        mock_run.assert_not_called()
        assert result == existing_file


class TestGenerateSingleDataset:
    """Test _generate_single_dataset subprocess invocation."""

    def test_returns_none_on_failure(self, tmp_path):
        output_dir = tmp_path / "synthetic"
        output_dir.mkdir()

        config = generate_benchmark_data.DATASET_CONFIGS[0]

        with patch("subprocess.run") as mock_run:
            mock_run.side_effect = subprocess.CalledProcessError(
                1, "cmd", output="out", stderr="err"
            )
            result = generate_benchmark_data._generate_single_dataset(config, output_dir)

        assert result is None

    def test_returns_path_on_success(self, tmp_path):
        output_dir = tmp_path / "synthetic"
        output_dir.mkdir()

        config = generate_benchmark_data.DATASET_CONFIGS[0]

        with patch("subprocess.run") as mock_run:
            mock_run.return_value = subprocess.CompletedProcess([], 0)
            result = generate_benchmark_data._generate_single_dataset(config, output_dir)

        assert result == output_dir / config["filename"]
        mock_run.assert_called_once()


class TestYamlAppend:
    """Test _append_datasets_to_yaml and _load_existing_labels."""

    def test_creates_new_yaml_if_missing(self, tmp_path):
        config_file = tmp_path / "benchmark_datasets.yaml"
        entries = [{"path": "integration/synthetic/foo.ntf", "label": "Foo"}]

        generate_benchmark_data._append_datasets_to_yaml(config_file, entries)

        with open(config_file) as fh:
            data = yaml.safe_load(fh)

        assert len(data["datasets"]) == 1
        assert data["datasets"][0]["label"] == "Foo"

    def test_appends_to_existing_yaml(self, tmp_path):
        config_file = tmp_path / "benchmark_datasets.yaml"
        initial = {"datasets": [{"path": "RANDOM/small.ntf", "label": "Tiny NITF"}]}
        with open(config_file, "w") as fh:
            yaml.dump(initial, fh)

        entries = [{"path": "integration/synthetic/foo.ntf", "label": "Foo"}]
        generate_benchmark_data._append_datasets_to_yaml(config_file, entries)

        with open(config_file) as fh:
            data = yaml.safe_load(fh)

        assert len(data["datasets"]) == 2
        labels = {e["label"] for e in data["datasets"]}
        assert labels == {"Tiny NITF", "Foo"}

    def test_skips_duplicate_labels(self, tmp_path):
        config_file = tmp_path / "benchmark_datasets.yaml"
        initial = {"datasets": [{"path": "old/path.ntf", "label": "Foo"}]}
        with open(config_file, "w") as fh:
            yaml.dump(initial, fh)

        entries = [{"path": "new/path.ntf", "label": "Foo"}]
        generate_benchmark_data._append_datasets_to_yaml(config_file, entries)

        with open(config_file) as fh:
            data = yaml.safe_load(fh)

        # Should still be just 1 entry — duplicate was skipped
        assert len(data["datasets"]) == 1

    def test_no_entries_is_noop(self, tmp_path):
        config_file = tmp_path / "benchmark_datasets.yaml"
        generate_benchmark_data._append_datasets_to_yaml(config_file, [])
        assert not config_file.exists()

    def test_load_existing_labels_empty_file(self, tmp_path):
        config_file = tmp_path / "benchmark_datasets.yaml"
        config_file.write_text("")
        labels = generate_benchmark_data._load_existing_labels(config_file)
        assert labels == set()

    def test_load_existing_labels_missing_file(self, tmp_path):
        config_file = tmp_path / "nonexistent.yaml"
        labels = generate_benchmark_data._load_existing_labels(config_file)
        assert labels == set()


class TestGenerateBenchmarkData:
    """Integration-level test for the main generate_benchmark_data function."""

    def test_creates_output_dir_and_appends_yaml(self, tmp_path):
        output_dir = tmp_path / "synthetic"
        config_file = tmp_path / "benchmark_datasets.yaml"

        # Mock subprocess to succeed for all calls
        with patch("subprocess.run") as mock_run:
            mock_run.return_value = subprocess.CompletedProcess([], 0)
            generate_benchmark_data.generate_benchmark_data(output_dir, config_file)

        # Output dir should have been created
        assert output_dir.exists()

        # YAML should have all 5 entries
        with open(config_file) as fh:
            data = yaml.safe_load(fh)

        assert len(data["datasets"]) == 5
        labels = {e["label"] for e in data["datasets"]}
        assert labels == {
            "Synth Small NC",
            "Synth Medium C3",
            "Synth Medium C8",
            "Synth Small TIFF",
            "Synth Large NC",
        }

        # Paths should be relative with integration/synthetic/ prefix
        for entry in data["datasets"]:
            assert entry["path"].startswith("synthetic/")

    def test_continues_on_single_failure(self, tmp_path):
        output_dir = tmp_path / "synthetic"
        config_file = tmp_path / "benchmark_datasets.yaml"

        call_count = 0

        def side_effect(*args, **kwargs):
            nonlocal call_count
            call_count += 1
            # Fail on the second call
            if call_count == 2:
                raise subprocess.CalledProcessError(1, "cmd", output="", stderr="err")
            return subprocess.CompletedProcess([], 0)

        with patch("subprocess.run", side_effect=side_effect):
            generate_benchmark_data.generate_benchmark_data(output_dir, config_file)

        with open(config_file) as fh:
            data = yaml.safe_load(fh)

        # 4 out of 5 should succeed
        assert len(data["datasets"]) == 4


class TestParseArgs:
    """Test CLI argument parsing."""

    def test_defaults(self):
        ns = generate_benchmark_data.parse_args([])
        assert isinstance(ns.output_dir, Path)
        assert isinstance(ns.config_file, Path)

    def test_custom_paths(self, tmp_path):
        ns = generate_benchmark_data.parse_args([
            "--output-dir", str(tmp_path / "out"),
            "--config-file", str(tmp_path / "cfg.yaml"),
        ])
        assert ns.output_dir == tmp_path / "out"
        assert ns.config_file == tmp_path / "cfg.yaml"
