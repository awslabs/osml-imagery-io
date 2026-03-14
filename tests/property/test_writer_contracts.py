"""Property-based tests for cross-format writer contracts.

These tests are parameterized over NITF and TIFF to verify universal
writer properties that apply to both formats:
- Property 4: Idempotent Close
- Property 6: Duplicate Key Rejection
- Property 7: Multi-Image IFD Ordering

**Validates: Requirements 1.3, 2.2, 2.4, 2.6**
"""

import tempfile
from pathlib import Path

import numpy as np
import pytest
from hypothesis import given, settings, Phase

from aws.osml.io import (
    IO,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    PixelType,
)

from .helpers import read_full_image
from .strategies import get_numpy_dtype, random_image


pbt_settings = settings(
    max_examples=100,
    deadline=None,
    phases=[Phase.explicit, Phase.reuse, Phase.generate, Phase.shrink],
    suppress_health_check=[],
)

# Format configurations: (extension, format_string, metadata_hints)
FORMAT_CONFIGS = [
    (".ntf", "nitf", {"IC": "NC"}),
    (".tif", "tiff", {}),
]

FORMAT_IDS = ["nitf", "tiff"]


def _make_provider(array, pixel_type, num_bands, num_rows, num_cols, key, hints):
    """Create a BufferedImageAssetProvider populated with the given array."""
    metadata = BufferedMetadataProvider()
    for k, v in hints.items():
        metadata.set(k, v)

    provider = BufferedImageAssetProvider.create(
        key=key,
        num_columns=num_cols,
        num_rows=num_rows,
        num_bands=num_bands,
        block_width=min(num_cols, 64),
        block_height=min(num_rows, 64),
        pixel_type=pixel_type,
        metadata=metadata,
    )
    provider.set_full_image(array)
    return provider, metadata


# =============================================================================
# Property 4: Idempotent Close (NITF + TIFF)
# =============================================================================


@pytest.mark.property
class TestIdempotentClose:
    """Property 4: Idempotent Close

    For any writer that has been populated with assets and closed, calling
    close() a second time returns successfully and the output file remains
    unchanged.

    # Feature: tiff-writing, Property 4: Idempotent close
    **Validates: Requirements 1.3**
    """

    @pytest.mark.parametrize("ext,fmt,hints", FORMAT_CONFIGS, ids=FORMAT_IDS)
    @given(data=random_image(min_size=16, max_size=48, min_bands=1, max_bands=3))
    @pbt_settings
    def test_idempotent_close(self, ext, fmt, hints, data):
        array, pixel_type, num_bands, num_rows, num_cols = data

        with tempfile.NamedTemporaryFile(suffix=ext, delete=False) as f:
            path = Path(f.name)

        try:
            provider, metadata = _make_provider(
                array, pixel_type, num_bands, num_rows, num_cols,
                "image_segment_0", hints,
            )

            writer = IO.open([str(path)], "w", fmt)
            writer.metadata = metadata
            writer.add_asset(
                key="image_segment_0",
                provider=provider,
                title="Test",
                description="",
                roles=["data"],
            )
            writer.close()

            bytes_after_first_close = path.read_bytes()

            # Second close should succeed and not change the file
            writer.close()

            bytes_after_second_close = path.read_bytes()
            assert bytes_after_first_close == bytes_after_second_close
        finally:
            path.unlink(missing_ok=True)


# =============================================================================
# Property 6: Duplicate Key Rejection (NITF + TIFF)
# =============================================================================

# Simplified format configs without hints (not needed for this test)
FORMAT_CONFIGS_SIMPLE = [
    (".ntf", "nitf"),
    (".tif", "tiff"),
]


@pytest.mark.property
class TestDuplicateKeyRejection:
    """Property 6: Duplicate Key Rejection

    For any key string, calling add_asset() twice with the same key shall
    succeed on the first call and raise an error on the second call.

    # Feature: tiff-writing, Property 6: Duplicate key rejection
    **Validates: Requirements 2.4**
    """

    @pytest.mark.parametrize("ext,fmt", FORMAT_CONFIGS_SIMPLE, ids=FORMAT_IDS)
    @given(data=random_image(min_size=16, max_size=32, min_bands=1, max_bands=1))
    @pbt_settings
    def test_duplicate_key_rejected(self, ext, fmt, data):
        array, pixel_type, num_bands, num_rows, num_cols = data

        hints = {"IC": "NC"} if fmt == "nitf" else {}
        provider1, metadata = _make_provider(
            array, pixel_type, num_bands, num_rows, num_cols,
            "image_segment_0", hints,
        )
        provider2, _ = _make_provider(
            array, pixel_type, num_bands, num_rows, num_cols,
            "image_segment_0", hints,
        )

        writer = IO.open([f"dup_test{ext}"], "w", fmt)
        writer.metadata = metadata

        # First add should succeed
        writer.add_asset(
            key="image_segment_0",
            provider=provider1,
            title="Image 0",
            description="",
            roles=["data"],
        )

        # Second add with same key should fail
        with pytest.raises(Exception):
            writer.add_asset(
                key="image_segment_0",
                provider=provider2,
                title="Image 0 dup",
                description="",
                roles=["data"],
            )


# =============================================================================
# Property 7: Multi-Image Ordering (NITF + TIFF)
# =============================================================================


@pytest.mark.property
class TestMultiImageOrdering:
    """Property 7: Multi-Image IFD Ordering

    For any sequence of N image assets added via add_asset(), the resulting
    file shall contain N images with pixel data matching insertion order.

    # Feature: tiff-writing, Property 7: Multi-image IFD ordering
    **Validates: Requirements 2.2, 2.6**
    """

    @pytest.mark.parametrize("ext,fmt,hints", FORMAT_CONFIGS, ids=FORMAT_IDS)
    @given(data=random_image(min_size=16, max_size=32, min_bands=1, max_bands=2))
    @pbt_settings
    def test_multi_image_ordering(self, ext, fmt, hints, data):
        array, pixel_type, num_bands, num_rows, num_cols = data

        # Generate 2–3 distinct images by shifting pixel values
        n_images = 2
        images = []
        for i in range(n_images):
            if np.issubdtype(array.dtype, np.floating):
                shifted = array + float(i)
            else:
                shifted = (array.astype(np.int64) + i).astype(array.dtype)
            images.append(shifted)

        with tempfile.NamedTemporaryFile(suffix=ext, delete=False) as f:
            path = Path(f.name)

        try:
            metadata = BufferedMetadataProvider()
            for k, v in hints.items():
                metadata.set(k, v)

            writer = IO.open([str(path)], "w", fmt)
            writer.metadata = metadata

            for i, img in enumerate(images):
                key = f"image_segment_{i}"
                provider, _ = _make_provider(
                    img, pixel_type, num_bands, num_rows, num_cols, key, hints,
                )
                writer.add_asset(
                    key=key,
                    provider=provider,
                    title=f"Image {i}",
                    description="",
                    roles=["data"],
                )

            writer.close()

            # Read back and verify ordering
            reader = IO.open([str(path)], "r")

            for i, expected in enumerate(images):
                key = f"image_segment_{i}"
                asset = reader.get_asset(key)
                decoded = read_full_image(asset, num_bands, num_rows, num_cols)

                assert decoded.shape == expected.shape, (
                    f"Image {i} shape mismatch"
                )
                np.testing.assert_array_equal(
                    decoded, expected,
                    err_msg=f"Image {i} pixel data mismatch",
                )
        finally:
            path.unlink(missing_ok=True)
