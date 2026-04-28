"""Tests for PyCallbackImageAssetProvider duck-typing support.

This module tests that Python-defined duck-typed objects can be passed to
DatasetWriter.add_asset() and correctly round-trip through the callback
adapter. It covers:

- Duck-typed provider acceptance by add_asset()
- Write-then-read roundtrip with known pixel data
- Missing required attribute rejection
- Python exception propagation from get_block()
- Default has_block behavior when method is absent
- Metadata dispatch (with and without get_metadata)
- Dtype mismatch error reporting

Requirements: 1.1, 1.2, 1.6, 2.4, 3.1, 3.3, 4.3, 5.1, 5.2, 7.1, 7.3, 8.1, 8.2
"""

import numpy as np
import pytest
from aws.osml.io import IO, BufferedMetadataProvider, PixelType

# ---------------------------------------------------------------------------
# Helper: minimal duck-typed provider
# ---------------------------------------------------------------------------


class DuckTypedProvider:
    """A plain Python class implementing the ImageAssetProvider interface.

    This is NOT a subclass of any Rust type — it relies entirely on duck
    typing to be accepted by DatasetWriter.add_asset().
    """

    def __init__(
        self,
        *,
        num_rows=64,
        num_cols=64,
        num_bands=1,
        pixel_type=PixelType.UInt8,
        block_width=None,
        block_height=None,
        data=None,
    ):
        self.key = "image:0"
        self.title = "Test Image"
        self.description = "Duck-typed test provider"
        self.num_rows = num_rows
        self.num_columns = num_cols
        self.num_bands = num_bands
        self.num_bits_per_pixel = self._bits_for(pixel_type)
        self.actual_bits_per_pixel = self._bits_for(pixel_type)
        self.pixel_value_type = pixel_type
        self.num_pixels_per_block_horizontal = block_width or num_cols
        self.num_pixels_per_block_vertical = block_height or num_rows
        self.num_resolution_levels = 1
        self.pad_pixel_value = 0.0

        dtype = np.dtype(pixel_type.to_numpy_dtype())
        if data is not None:
            self._data = data
        else:
            self._data = np.zeros(
                (num_bands, num_rows, num_cols), dtype=dtype
            )

    @staticmethod
    def _bits_for(pixel_type):
        name = pixel_type.to_numpy_dtype()
        bits = np.dtype(name).itemsize * 8
        return bits

    def get_block(self, block_row, block_col, resolution_level, bands=None):
        bh = self.num_pixels_per_block_vertical
        bw = self.num_pixels_per_block_horizontal
        r0 = block_row * bh
        c0 = block_col * bw
        r1 = min(r0 + bh, self.num_rows)
        c1 = min(c0 + bw, self.num_columns)
        return self._data[:, r0:r1, c0:c1].copy()


def _write_nitf(tmp_path, provider):
    """Write a NITF file using the given provider and return the output path."""
    output_path = str(tmp_path / "output.ntf")
    metadata = BufferedMetadataProvider()
    metadata.set("IC", "NC")

    with IO.open([output_path], "w", "nitf") as writer:
        writer.metadata = metadata
        writer.add_asset(
            "image:0", provider, "Test", "Test image", ["data"]
        )

    return output_path


# =========================================================================
# Test 1: duck-typed provider accepted by DatasetWriter.add_asset()
# =========================================================================


class TestDuckTypedProviderAccepted:
    """Verify that a plain Python object with the right attributes is accepted.

    Requirements: 3.1, 7.1
    """

    def test_duck_typed_provider_accepted(self, tmp_path):
        """A duck-typed provider should be accepted by add_asset() without error."""
        provider = DuckTypedProvider()
        _write_nitf(tmp_path, provider)
        # If we get here without an exception, the provider was accepted
        assert (tmp_path / "output.ntf").exists()


# =========================================================================
# Test 2: write-then-read roundtrip
# =========================================================================


class TestWriteReadRoundtrip:
    """Verify pixel data survives a write-then-read roundtrip.

    Requirements: 1.1, 1.2, 7.1, 7.3
    """

    def test_roundtrip_uint8_data_matches(self, tmp_path):
        """Write known UInt8 block data via duck-typed provider, read back, verify match."""
        # Create a known pattern: ascending values
        data = np.arange(64 * 64, dtype=np.uint8).reshape(1, 64, 64)
        provider = DuckTypedProvider(data=data)

        output_path = _write_nitf(tmp_path, provider)

        with IO.open([output_path], "r") as reader:
            asset = reader.get_asset("image:0")
            block = asset.get_block(0, 0, 0)

        np.testing.assert_array_equal(block, data)


# =========================================================================
# Test 3: missing required attribute
# =========================================================================


class TestMissingRequiredAttribute:
    """Verify that a provider missing a required attribute is rejected.

    Requirements: 2.4, 3.3
    """

    def test_missing_num_rows_raises_type_error(self, tmp_path):
        """A provider missing num_rows should be rejected with TypeError."""
        provider = DuckTypedProvider()
        del provider.num_rows

        with pytest.raises(TypeError):
            _write_nitf(tmp_path, provider)


# =========================================================================
# Test 4: get_block() exception propagation
# =========================================================================


class TestGetBlockExceptionPropagation:
    """Verify that Python exceptions in get_block() produce RuntimeError.

    Requirements: 5.1, 5.2
    """

    def test_get_block_exception_produces_runtime_error(self, tmp_path):
        """A provider whose get_block() raises should produce RuntimeError."""

        class FailingProvider(DuckTypedProvider):
            def get_block(self, block_row, block_col, resolution_level, bands=None):
                raise ValueError("Simulated processing failure")

        provider = FailingProvider()

        with pytest.raises(RuntimeError, match="Simulated processing failure"):
            _write_nitf(tmp_path, provider)


# =========================================================================
# Test 5: provider without has_block method
# =========================================================================


class TestNoHasBlockMethod:
    """Verify that a provider without has_block works (default true behavior).

    Requirements: 1.6
    """

    def test_provider_without_has_block_works(self, tmp_path):
        """A provider without has_block should still write successfully."""
        provider = DuckTypedProvider()
        # DuckTypedProvider does not define has_block — that's the point
        assert not hasattr(provider, "has_block")

        output_path = _write_nitf(tmp_path, provider)

        with IO.open([output_path], "r") as reader:
            asset = reader.get_asset("image:0")
            block = asset.get_block(0, 0, 0)

        assert block.shape == (1, 64, 64)


# =========================================================================
# Test 6: provider with get_metadata method
# =========================================================================


class TestProviderWithGetMetadata:
    """Verify that a provider's get_metadata is called and metadata passes through.

    Requirements: 8.1
    """

    def test_get_metadata_passed_through(self, tmp_path):
        """Metadata from the provider's get_metadata should be accessible on the asset."""

        class MetadataProvider(DuckTypedProvider):
            def get_metadata(self):
                meta = BufferedMetadataProvider()
                meta.set("IREP", "MONO")
                return meta

        provider = MetadataProvider()
        output_path = _write_nitf(tmp_path, provider)

        with IO.open([output_path], "r") as reader:
            asset = reader.get_asset("image:0")
            meta_dict = asset.get_metadata().as_dict()

        # The IREP value from the provider's metadata should be present
        assert meta_dict.get("IREP") == "MONO"


# =========================================================================
# Test 7: provider without get_metadata method
# =========================================================================


class TestProviderWithoutGetMetadata:
    """Verify that a provider without get_metadata uses empty metadata default.

    Requirements: 8.2
    """

    def test_no_get_metadata_uses_empty_default(self, tmp_path):
        """A provider without get_metadata should still write successfully."""
        provider = DuckTypedProvider()
        assert not hasattr(provider, "get_metadata")

        output_path = _write_nitf(tmp_path, provider)

        # The file should be readable — empty metadata default was used
        with IO.open([output_path], "r") as reader:
            asset = reader.get_asset("image:0")
            assert asset is not None
            # Should be able to read data back
            block = asset.get_block(0, 0, 0)
            assert block.shape == (1, 64, 64)


# =========================================================================
# Test 8: dtype mismatch
# =========================================================================


class TestDtypeMismatch:
    """Verify that a dtype mismatch produces an error describing the mismatch.

    Requirements: 4.3
    """

    def test_dtype_mismatch_produces_error(self, tmp_path):
        """Provider declares UInt8 but get_block() returns float32 → error."""

        class MismatchProvider(DuckTypedProvider):
            def get_block(self, block_row, block_col, resolution_level, bands=None):
                # Return float32 data even though pixel_value_type is UInt8
                return np.zeros((1, 64, 64), dtype=np.float32)

        provider = MismatchProvider(pixel_type=PixelType.UInt8)

        with pytest.raises(RuntimeError, match="dtype"):
            _write_nitf(tmp_path, provider)
