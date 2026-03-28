"""Unit tests for TileIndex: save, load, generate, _detect_format, and properties."""

from __future__ import annotations

import json
import os

import pytest
from aws.osml.io.tile_index import TileIndex, _detect_format


def _make_refs() -> dict:
    """Build a minimal valid Kerchunk v1 refs dict for testing."""
    return {
        "version": 1,
        "refs": {
            ".zgroup": json.dumps({"zarr_format": 3}),
            ".zattrs": json.dumps({"source": "s3://bucket/image.ntf", "format": "nitf"}),
            "image_segment_0/.zarray": json.dumps({
                "shape": [3, 256, 256],
                "chunks": [3, 64, 64],
                "data_type": "uint8",
            }),
            "image_segment_0/0.0": ["s3://bucket/image.ntf", 1024, 4096],
            "image_segment_0/0.1": ["s3://bucket/image.ntf", 5120, 4096],
            "image_segment_0/1.0": ["s3://bucket/image.ntf", 9216, 4096],
            "image_segment_0/1.1": ["s3://bucket/image.ntf", 13312, 4096],
        },
    }


class TestSaveUnsupportedExtension:
    """Requirement 11.5: ValueError for unsupported extensions."""

    def test_unsupported_extension_raises(self, tmp_path):
        idx = TileIndex(_make_refs())
        with pytest.raises(ValueError, match=r"Unsupported file extension '.csv'"):
            idx.save(str(tmp_path / "index.csv"))

    def test_unsupported_extension_no_ext(self, tmp_path):
        idx = TileIndex(_make_refs())
        with pytest.raises(ValueError, match=r"Unsupported file extension"):
            idx.save(str(tmp_path / "index"))

    def test_unsupported_extension_xml(self, tmp_path):
        idx = TileIndex(_make_refs())
        with pytest.raises(ValueError, match=r"Supported: .json, .parquet"):
            idx.save(str(tmp_path / "index.xml"))


class TestSaveJson:
    """Requirements 5.1, 5.2, 5.3: JSON serialization."""

    def test_save_json_creates_file(self, tmp_path):
        idx = TileIndex(_make_refs())
        out = str(tmp_path / "index.json")
        idx.save(out)
        assert os.path.exists(out)

    def test_save_json_valid_json(self, tmp_path):
        idx = TileIndex(_make_refs())
        out = str(tmp_path / "index.json")
        idx.save(out)
        with open(out) as f:
            data = json.load(f)
        assert data["version"] == 1
        assert "refs" in data

    def test_save_json_preserves_refs(self, tmp_path):
        refs = _make_refs()
        idx = TileIndex(refs)
        out = str(tmp_path / "index.json")
        idx.save(out)
        with open(out) as f:
            loaded = json.load(f)
        assert loaded == refs

    def test_save_json_preserves_large_offsets(self, tmp_path):
        """Requirement 5.3: int precision up to 2^53."""
        large_offset = 2**53 - 1
        refs = _make_refs()
        refs["refs"]["image_segment_0/0.0"] = ["s3://bucket/img.ntf", large_offset, 4096]
        idx = TileIndex(refs)
        out = str(tmp_path / "index.json")
        idx.save(out)
        with open(out) as f:
            loaded = json.load(f)
        assert loaded["refs"]["image_segment_0/0.0"][1] == large_offset


class TestSaveParquet:
    """Requirements 6.1, 6.2, 6.3, 6.4: Parquet serialization."""

    def test_save_parquet_creates_file(self, tmp_path):
        idx = TileIndex(_make_refs())
        out = str(tmp_path / "index.parquet")
        idx.save(out)
        assert os.path.exists(out)

    def test_save_parquet_tile_refs_in_table(self, tmp_path):
        """Tile refs stored as table rows with correct columns."""
        import pyarrow.parquet as pq

        idx = TileIndex(_make_refs())
        out = str(tmp_path / "index.parquet")
        idx.save(out)

        table = pq.read_table(out)
        assert set(table.column_names) == {"path", "url", "offset", "size"}
        assert table.num_rows == 4  # 4 tile refs in _make_refs()

    def test_save_parquet_int64_columns(self, tmp_path):
        """Requirement 6.4: offset and size are int64."""
        import pyarrow as pa
        import pyarrow.parquet as pq

        idx = TileIndex(_make_refs())
        out = str(tmp_path / "index.parquet")
        idx.save(out)

        table = pq.read_table(out)
        assert table.schema.field("offset").type == pa.int64()
        assert table.schema.field("size").type == pa.int64()

    def test_save_parquet_inline_metadata_in_footer(self, tmp_path):
        """Requirement 6.2: inline metadata in Parquet file metadata."""
        import pyarrow.parquet as pq

        idx = TileIndex(_make_refs())
        out = str(tmp_path / "index.parquet")
        idx.save(out)

        pf = pq.ParquetFile(out)
        file_meta = pf.schema_arrow.metadata
        assert b".zgroup" in file_meta
        assert b".zattrs" in file_meta
        assert b"image_segment_0/.zarray" in file_meta

    def test_save_parquet_metadata_values_match(self, tmp_path):
        """Inline metadata values match the original refs."""
        import pyarrow.parquet as pq

        refs = _make_refs()
        idx = TileIndex(refs)
        out = str(tmp_path / "index.parquet")
        idx.save(out)

        pf = pq.ParquetFile(out)
        file_meta = pf.schema_arrow.metadata
        assert file_meta[b".zgroup"] == refs["refs"][".zgroup"].encode("utf-8")
        assert file_meta[b".zattrs"] == refs["refs"][".zattrs"].encode("utf-8")

    def test_save_parquet_tile_data_correct(self, tmp_path):
        """Tile reference data matches the original refs."""
        import pyarrow.parquet as pq

        refs = _make_refs()
        idx = TileIndex(refs)
        out = str(tmp_path / "index.parquet")
        idx.save(out)

        table = pq.read_table(out)
        df = table.to_pydict()

        # Build expected tile refs from the original
        expected_tiles = {
            k: v for k, v in refs["refs"].items()
            if isinstance(v, list) and len(v) == 3
        }

        for i, path in enumerate(df["path"]):
            assert path in expected_tiles
            assert df["url"][i] == expected_tiles[path][0]
            assert df["offset"][i] == expected_tiles[path][1]
            assert df["size"][i] == expected_tiles[path][2]

    def test_save_parquet_version_in_metadata(self, tmp_path):
        """Version is stored in Parquet file metadata."""
        import pyarrow.parquet as pq

        idx = TileIndex(_make_refs())
        out = str(tmp_path / "index.parquet")
        idx.save(out)

        pf = pq.ParquetFile(out)
        file_meta = pf.schema_arrow.metadata
        assert file_meta[b"version"] == b"1"

    def test_save_parquet_empty_tiles(self, tmp_path):
        """Index with only metadata and no tile refs produces empty table."""
        import pyarrow.parquet as pq

        refs = {
            "version": 1,
            "refs": {
                ".zgroup": json.dumps({"zarr_format": 3}),
                ".zattrs": json.dumps({"source": "s3://bucket/img.ntf"}),
            },
        }
        idx = TileIndex(refs)
        out = str(tmp_path / "index.parquet")
        idx.save(out)

        table = pq.read_table(out)
        assert table.num_rows == 0

        pf = pq.ParquetFile(out)
        file_meta = pf.schema_arrow.metadata
        assert b".zgroup" in file_meta


class TestSaveParquetImportError:
    """Requirement 11.6: ImportError when pyarrow is missing."""

    def test_import_error_message(self, tmp_path, monkeypatch):
        import builtins

        real_import = builtins.__import__

        def mock_import(name, *args, **kwargs):
            if name == "pyarrow":
                raise ImportError("No module named 'pyarrow'")
            return real_import(name, *args, **kwargs)

        monkeypatch.setattr(builtins, "__import__", mock_import)

        idx = TileIndex(_make_refs())
        with pytest.raises(ImportError, match="pyarrow is required for Parquet output"):
            idx.save(str(tmp_path / "index.parquet"))


# ---------------------------------------------------------------------------
# Tests for TileIndex.load()
# ---------------------------------------------------------------------------


class TestLoadFileNotFound:
    """Requirement 7.4: FileNotFoundError when path does not exist."""

    def test_load_nonexistent_json(self):
        with pytest.raises(FileNotFoundError, match="File not found: /no/such/file.json"):
            TileIndex.load("/no/such/file.json")

    def test_load_nonexistent_parquet(self):
        with pytest.raises(FileNotFoundError, match="File not found: /no/such/file.parquet"):
            TileIndex.load("/no/such/file.parquet")


class TestLoadUnsupportedExtension:
    """Requirement 11.5: ValueError for unsupported extensions on load."""

    def test_unsupported_extension_csv(self, tmp_path):
        p = tmp_path / "index.csv"
        p.write_text("")
        with pytest.raises(ValueError, match=r"Unsupported file extension '.csv'"):
            TileIndex.load(str(p))

    def test_unsupported_extension_no_ext(self, tmp_path):
        p = tmp_path / "index"
        p.write_text("")
        with pytest.raises(ValueError, match=r"Unsupported file extension"):
            TileIndex.load(str(p))

    def test_unsupported_extension_xml(self, tmp_path):
        p = tmp_path / "index.xml"
        p.write_text("")
        with pytest.raises(ValueError, match=r"Supported: .json, .parquet"):
            TileIndex.load(str(p))


class TestLoadJson:
    """Requirements 7.1, 7.3: JSON deserialization and version validation."""

    def test_load_json_round_trip(self, tmp_path):
        """Save then load produces identical refs."""
        refs = _make_refs()
        idx = TileIndex(refs)
        out = str(tmp_path / "index.json")
        idx.save(out)

        loaded = TileIndex.load(out)
        assert loaded.refs == refs

    def test_load_json_version_validation(self, tmp_path):
        """Requirement 7.3: wrong version raises ValueError."""
        bad = {"version": 2, "refs": {}}
        out = str(tmp_path / "bad.json")
        with open(out, "w") as f:
            json.dump(bad, f)

        with pytest.raises(ValueError, match=r"expected version 1, got 2"):
            TileIndex.load(out)

    def test_load_json_missing_version(self, tmp_path):
        """Requirement 7.3: missing version raises ValueError."""
        bad = {"refs": {}}
        out = str(tmp_path / "bad.json")
        with open(out, "w") as f:
            json.dump(bad, f)

        with pytest.raises(ValueError, match=r"expected version 1, got None"):
            TileIndex.load(out)

    def test_load_json_preserves_properties(self, tmp_path):
        """Loaded index has correct num_segments and num_tiles."""
        refs = _make_refs()
        idx = TileIndex(refs)
        out = str(tmp_path / "index.json")
        idx.save(out)

        loaded = TileIndex.load(out)
        assert loaded.num_segments == 1
        assert loaded.num_tiles == 4


class TestLoadParquet:
    """Requirements 7.2, 7.3: Parquet deserialization and version validation."""

    def test_load_parquet_round_trip(self, tmp_path):
        """Save then load produces identical refs."""
        refs = _make_refs()
        idx = TileIndex(refs)
        out = str(tmp_path / "index.parquet")
        idx.save(out)

        loaded = TileIndex.load(out)
        assert loaded.refs["version"] == 1
        assert loaded.num_segments == 1
        assert loaded.num_tiles == 4

        # Verify all tile refs match
        for key, value in refs["refs"].items():
            if isinstance(value, list) and len(value) == 3:
                assert loaded.refs["refs"][key] == value

    def test_load_parquet_inline_metadata_preserved(self, tmp_path):
        """Inline metadata (.zgroup, .zattrs, .zarray) survives round-trip."""
        refs = _make_refs()
        idx = TileIndex(refs)
        out = str(tmp_path / "index.parquet")
        idx.save(out)

        loaded = TileIndex.load(out)
        assert loaded.refs["refs"][".zgroup"] == refs["refs"][".zgroup"]
        assert loaded.refs["refs"][".zattrs"] == refs["refs"][".zattrs"]
        assert loaded.refs["refs"]["image_segment_0/.zarray"] == refs["refs"]["image_segment_0/.zarray"]

    def test_load_parquet_version_validation(self, tmp_path):
        """Requirement 7.3: wrong version in Parquet raises ValueError."""
        import pyarrow as pa
        import pyarrow.parquet as pq

        schema = pa.schema(
            [
                pa.field("path", pa.string()),
                pa.field("url", pa.string()),
                pa.field("offset", pa.int64()),
                pa.field("size", pa.int64()),
            ],
            metadata={b"version": b"2"},
        )
        table = pa.table(
            {"path": [], "url": [], "offset": [], "size": []},
            schema=schema,
        )
        out = str(tmp_path / "bad.parquet")
        pq.write_table(table, out)

        with pytest.raises(ValueError, match=r"expected version 1, got 2"):
            TileIndex.load(out)

    def test_load_parquet_preserves_large_offsets(self, tmp_path):
        """Large int64 offsets survive Parquet round-trip."""
        large_offset = 2**53 - 1
        refs = _make_refs()
        refs["refs"]["image_segment_0/0.0"] = ["s3://bucket/img.ntf", large_offset, 4096]
        idx = TileIndex(refs)
        out = str(tmp_path / "index.parquet")
        idx.save(out)

        loaded = TileIndex.load(out)
        assert loaded.refs["refs"]["image_segment_0/0.0"][1] == large_offset

    def test_load_parquet_empty_tiles(self, tmp_path):
        """Index with only metadata and no tile refs loads correctly."""
        refs = {
            "version": 1,
            "refs": {
                ".zgroup": json.dumps({"zarr_format": 3}),
                ".zattrs": json.dumps({"source": "s3://bucket/img.ntf"}),
            },
        }
        idx = TileIndex(refs)
        out = str(tmp_path / "index.parquet")
        idx.save(out)

        loaded = TileIndex.load(out)
        assert loaded.refs["version"] == 1
        assert loaded.num_tiles == 0
        assert loaded.refs["refs"][".zgroup"] == refs["refs"][".zgroup"]


class TestLoadParquetImportError:
    """Requirement 11.6: ImportError when pyarrow is missing for load."""

    def test_import_error_on_load(self, tmp_path, monkeypatch):
        import builtins

        # First save a valid parquet file before mocking
        refs = _make_refs()
        idx = TileIndex(refs)
        out = str(tmp_path / "index.parquet")
        idx.save(out)

        real_import = builtins.__import__

        def mock_import(name, *args, **kwargs):
            if name == "pyarrow":
                raise ImportError("No module named 'pyarrow'")
            return real_import(name, *args, **kwargs)

        monkeypatch.setattr(builtins, "__import__", mock_import)

        with pytest.raises(ImportError, match="pyarrow is required for Parquet output"):
            TileIndex.load(out)


# ---------------------------------------------------------------------------
# Tests for TileIndex.generate() error handling
# ---------------------------------------------------------------------------


class TestGenerateFileNotFound:
    """Requirement 11.1: FileNotFoundError when path does not exist."""

    def test_generate_nonexistent_path(self):
        with pytest.raises(FileNotFoundError):
            TileIndex.generate("/no/such/file.ntf", source_uri="s3://bucket/img.ntf")


class TestGenerateKeyError:
    """Requirement 11.4: KeyError for unrecognized segment key."""

    def test_unrecognized_segment_key(self):
        path = os.path.join(os.path.dirname(__file__), "..", "data", "unit", "i_3001a.ntf")
        if not os.path.exists(path):
            pytest.skip(f"Test data file not found: {path}")
        with pytest.raises(KeyError):
            TileIndex.generate(path, source_uri="s3://bucket/img.ntf", segments=["nonexistent_segment"])


# ---------------------------------------------------------------------------
# Tests for _detect_format()
# ---------------------------------------------------------------------------


class TestDetectFormat:
    """Requirement 4.3: Format detection from file extension."""

    @pytest.mark.parametrize(
        "ext, expected",
        [
            (".ntf", "nitf"),
            (".nitf", "nitf"),
            (".nsif", "nitf"),
            (".nsf", "nitf"),
            (".tif", "tiff"),
            (".tiff", "tiff"),
            (".gtif", "tiff"),
            (".gtiff", "tiff"),
            (".j2k", "jpeg2000"),
            (".jp2", "jpeg2000"),
            (".jpg", "jpeg"),
            (".jpeg", "jpeg"),
            (".png", "png"),
        ],
    )
    def test_known_extensions(self, ext, expected):
        assert _detect_format(f"image{ext}") == expected

    @pytest.mark.parametrize("ext", [".bmp", ".gif", ".csv", ""])
    def test_unknown_extensions_return_none(self, ext):
        assert _detect_format(f"file{ext}") is None

    def test_case_insensitivity_ntf(self):
        assert _detect_format("IMAGE.NTF") == "nitf"

    def test_case_insensitivity_tiff(self):
        assert _detect_format("IMAGE.TIFF") == "tiff"


# ---------------------------------------------------------------------------
# Tests for num_segments and num_tiles properties
# ---------------------------------------------------------------------------


class TestNumSegmentsAndNumTiles:
    """Requirements 10.6, 10.7: num_segments and num_tiles properties."""

    def test_zero_segments_zero_tiles(self):
        idx = TileIndex({"version": 1, "refs": {}})
        assert idx.num_segments == 0
        assert idx.num_tiles == 0

    def test_one_segment_four_tiles(self):
        refs = _make_refs()
        idx = TileIndex(refs)
        assert idx.num_segments == 1
        assert idx.num_tiles == 4

    def test_two_segments_multiple_tiles(self):
        refs = _make_refs()
        # Add a second segment
        refs["refs"]["image_segment_1/.zarray"] = json.dumps({
            "shape": [1, 128, 128],
            "chunks": [1, 64, 64],
            "data_type": "uint16",
        })
        refs["refs"]["image_segment_1/0.0"] = ["s3://bucket/image.ntf", 20000, 2048]
        refs["refs"]["image_segment_1/0.1"] = ["s3://bucket/image.ntf", 22048, 2048]
        idx = TileIndex(refs)
        assert idx.num_segments == 2
        assert idx.num_tiles == 6

    def test_non_list_values_not_counted_as_tiles(self):
        """String values (inline metadata) should not be counted as tiles."""
        refs = {
            "version": 1,
            "refs": {
                ".zgroup": json.dumps({"zarr_format": 3}),
                ".zattrs": json.dumps({"source": "s3://bucket/img.ntf"}),
            },
        }
        idx = TileIndex(refs)
        assert idx.num_tiles == 0

    def test_lists_with_wrong_length_not_counted(self):
        """Lists with != 3 elements should not be counted as tiles."""
        refs = {
            "version": 1,
            "refs": {
                "two_elem": ["s3://bucket/img.ntf", 100],
                "four_elem": ["s3://bucket/img.ntf", 100, 200, 300],
                "one_elem": ["s3://bucket/img.ntf"],
                "valid_tile/0.0": ["s3://bucket/img.ntf", 0, 4096],
            },
        }
        idx = TileIndex(refs)
        assert idx.num_tiles == 1
