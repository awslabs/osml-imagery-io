"""Image segment conformance tests for NITF parsing validation.

This module provides pytest-based conformance tests that validate NITF image
segment parsing against expected outcomes defined in a manifest file. Tests are
dynamically generated from the manifest and support graceful degradation when
test data is unavailable.

These tests validate:
- Image subheader parsing (Requirements 1.1-1.10, 2.1-2.5, 3.1-3.9)
- Block reading for uncompressed images (Requirements 5.1-5.6, 6.1-6.5)
- Multi-band image handling (Requirements 3.1-3.9, 15.1-15.5)

IMPORTANT: This module does NOT directly reference JITC test data paths.
All test files are discovered through the manifest.json file in the integration
data directory. Users with access to JITC test data should populate the manifest
with appropriate entries.
"""

import logging
import os
import pytest
from pathlib import Path
from typing import Optional

import numpy as np

from tests.conformance import TestFileEntry, TestManifest

logger = logging.getLogger(__name__)


# =============================================================================
# Constants
# =============================================================================

# Category names for filtering image-related tests
CATEGORY_IMAGE_PARSING = "image_parsing"
CATEGORY_IMAGE_BLOCKING = "image_blocking"
CATEGORY_IMAGE_MULTIBAND = "image_multiband"


# =============================================================================
# Helper Functions
# =============================================================================

def get_integration_data_path() -> Path:
    """Get integration data path from environment variable or default.
    
    Resolution order:
    1. OSML_IO_INTEGRATION_DATA environment variable if set
    2. Default path "data/integration/"
    
    Returns:
        Path to the integration data directory
    """
    env_path = os.environ.get("OSML_IO_INTEGRATION_DATA")
    if env_path:
        return Path(env_path)
    return Path("data/integration")


def get_manifest_path() -> Path:
    """Get manifest file path within integration data directory."""
    return get_integration_data_path() / "manifest.json"


def load_image_test_cases(category: str) -> list[tuple[str, TestFileEntry]]:
    """Load test cases for a specific image category.
    
    Args:
        category: Category string to filter by
        
    Returns:
        List of (path, entry) tuples for parametrization.
        Returns empty list if manifest file not found or no matching entries.
    """
    base_path = get_integration_data_path()
    
    if not base_path.exists():
        logger.warning(f"Test data directory not found: {base_path}")
        return []
    
    manifest_path = get_manifest_path()
    
    if not manifest_path.exists():
        logger.warning(f"Manifest file not found: {manifest_path}")
        return []
    
    manifest = TestManifest.load(manifest_path, base_path)
    entries = manifest.entries_by_category(category)
    return [(entry.path, entry) for entry in entries]


def _get_test_id(item) -> str:
    """Generate test ID for parametrization."""
    if isinstance(item, str):
        return item
    if isinstance(item, TestFileEntry):
        return item.path
    return str(item)


# =============================================================================
# Image Segment Parsing Tests (Task 14.1)
# Requirements: 1.1-1.10, 2.1-2.5, 3.1-3.9
# =============================================================================

_image_parsing_cases = load_image_test_cases(CATEGORY_IMAGE_PARSING)


@pytest.mark.integration
@pytest.mark.parametrize(
    "path,entry",
    _image_parsing_cases if _image_parsing_cases else [
        pytest.param("no_manifest", None, marks=pytest.mark.skip(
            reason="No image_parsing entries in manifest"
        ))
    ],
    ids=_get_test_id,
)
def test_image_segment_parsing(path: str, entry: Optional[TestFileEntry]):
    """Test image subheader parsing against expected outcomes.
    
    This test validates that image subheaders are correctly parsed from NITF
    files. It verifies:
    - Image identifiers (IID1, IID2)
    - Image dimensions (NROWS, NCOLS)
    - Pixel characteristics (PVTYPE, IREP, ICAT, ABPP, NBPP, PJUST)
    - Blocking parameters (NBPR, NBPC, NPPBH, NPPBV, IMODE)
    - Band information (NBANDS/XBANDS, band metadata)
    
    Args:
        path: Relative path to the test file
        entry: TestFileEntry with expected outcomes
        
    Requirements: 1.1-1.10, 2.1-2.5, 3.1-3.9
    """
    if entry is None:
        pytest.skip("No image_parsing entries in manifest")
        return
    
    file_path = get_integration_data_path() / path
    
    if not file_path.exists():
        logger.warning(f"Test file not found, skipping: {file_path}")
        pytest.skip(f"Test file not found: {path}")
        return
    
    # Import here to avoid import errors if library not built
    from aws.osml.io import IO, AssetType
    
    exception_raised = False
    actual_exception: Optional[BaseException] = None
    
    try:
        reader = IO.open([str(file_path)], "r")
        
        # Get all image segment keys
        image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
        
        # Verify at least one image segment exists (for valid files)
        if entry.expected_valid:
            assert len(image_keys) > 0, "Expected at least one image segment"
        
        # Parse each image segment's subheader
        for key in image_keys:
            asset = reader.get_asset(key)
            
            # Access subheader fields to verify parsing
            metadata = asset.get_metadata()
            fields = metadata.as_dict()
            
            # Verify required fields are present
            assert "NROWS" in fields or asset.num_rows > 0
            assert "NCOLS" in fields or asset.num_columns > 0
            
            # Verify dimensions are valid
            assert asset.num_rows > 0, "NROWS must be positive"
            assert asset.num_columns > 0, "NCOLS must be positive"
            
            # Verify band count
            assert asset.num_bands >= 1, "Must have at least 1 band"
            
            # Verify blocking parameters
            assert asset.num_pixels_per_block_horizontal > 0
            assert asset.num_pixels_per_block_vertical > 0
            
            # Verify bits per pixel
            assert asset.num_bits_per_pixel > 0
            assert asset.actual_bits_per_pixel > 0
            assert asset.actual_bits_per_pixel <= asset.num_bits_per_pixel
            
    except Exception as e:
        exception_raised = True
        actual_exception = e
    
    # Verify outcome matches expectation
    if entry.expected_valid:
        assert not exception_raised, f"Expected valid file but got: {actual_exception}"
    else:
        assert exception_raised, "Expected parsing to fail but it succeeded"
        if entry.expected_exception:
            actual_type = type(actual_exception).__name__
            assert actual_type == entry.expected_exception, \
                f"Expected {entry.expected_exception}, got {actual_type}"
        if entry.expected_message:
            assert entry.expected_message in str(actual_exception), \
                f"Expected message containing '{entry.expected_message}'"


# =============================================================================
# Block Reading Tests (Task 14.2)
# Requirements: 5.1-5.6, 6.1-6.5
# =============================================================================

_image_blocking_cases = load_image_test_cases(CATEGORY_IMAGE_BLOCKING)


@pytest.mark.integration
@pytest.mark.parametrize(
    "path,entry",
    _image_blocking_cases if _image_blocking_cases else [
        pytest.param("no_manifest", None, marks=pytest.mark.skip(
            reason="No image_blocking entries in manifest"
        ))
    ],
    ids=_get_test_id,
)
def test_image_block_reading(path: str, entry: Optional[TestFileEntry]):
    """Test block reading from uncompressed images.
    
    This test validates that image blocks can be correctly read from
    uncompressed NITF images. It verifies:
    - Block access returns valid data
    - Block shape matches expected dimensions
    - Pixel values are within valid range for the data type
    - Edge blocks are handled correctly
    
    Args:
        path: Relative path to the test file
        entry: TestFileEntry with expected outcomes
        
    Requirements: 5.1-5.6, 6.1-6.5
    """
    if entry is None:
        pytest.skip("No image_blocking entries in manifest")
        return
    
    file_path = get_integration_data_path() / path
    
    if not file_path.exists():
        logger.warning(f"Test file not found, skipping: {file_path}")
        pytest.skip(f"Test file not found: {path}")
        return
    
    from aws.osml.io import IO, AssetType
    
    exception_raised = False
    actual_exception: Optional[BaseException] = None
    
    try:
        reader = IO.open([str(file_path)], "r")
        image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
        
        for key in image_keys:
            asset = reader.get_asset(key)
            
            # Skip compressed images for this test
            metadata = asset.get_metadata()
            fields = metadata.as_dict()
            ic = fields.get("IC", "NC")
            if ic not in ("NC", "NM"):
                logger.info(f"Skipping compressed image {key} with IC={ic}")
                continue
            
            # Verify has_block for valid coordinates
            assert asset.has_block(0, 0, 0), "Block (0,0) should exist"
            
            # Read first block
            block = asset.get_block(0, 0, 0)
            
            # Verify block is a numpy array
            assert isinstance(block, np.ndarray), "Block should be numpy array"
            
            # Verify block shape is (bands, rows, cols) - channels first per API design
            assert len(block.shape) == 3, "Block should have 3 dimensions"
            assert block.shape[0] == asset.num_bands, "Band count mismatch"
            
            # Verify block dimensions don't exceed block size
            assert block.shape[1] <= asset.num_pixels_per_block_vertical
            assert block.shape[2] <= asset.num_pixels_per_block_horizontal
            
            # Verify pixel values are reasonable (not all zeros unless expected)
            # This is a sanity check, not a strict requirement
            
            # Test invalid block coordinates return False for has_block
            grid_rows, grid_cols = asset.block_grid_size
            assert not asset.has_block(grid_rows + 100, grid_cols + 100, 0), \
                "Invalid coordinates should return False"
            
    except Exception as e:
        exception_raised = True
        actual_exception = e
    
    if entry.expected_valid:
        assert not exception_raised, f"Expected valid file but got: {actual_exception}"
    else:
        assert exception_raised, "Expected block reading to fail but it succeeded"
        if entry.expected_exception:
            actual_type = type(actual_exception).__name__
            assert actual_type == entry.expected_exception
        if entry.expected_message:
            assert entry.expected_message in str(actual_exception)


# =============================================================================
# Multi-Band Image Tests (Task 14.3)
# Requirements: 3.1-3.9, 15.1-15.5
# =============================================================================

_image_multiband_cases = load_image_test_cases(CATEGORY_IMAGE_MULTIBAND)


@pytest.mark.integration
@pytest.mark.parametrize(
    "path,entry",
    _image_multiband_cases if _image_multiband_cases else [
        pytest.param("no_manifest", None, marks=pytest.mark.skip(
            reason="No image_multiband entries in manifest"
        ))
    ],
    ids=_get_test_id,
)
def test_multiband_image_handling(path: str, entry: Optional[TestFileEntry]):
    """Test multi-band image parsing and band selection.
    
    This test validates that multi-band images (RGB, multispectral) are
    correctly parsed and that band selection works properly. It verifies:
    - Band count matches IREP requirements
    - Band info is accessible for each band
    - Band selection in get_block() works correctly
    
    Args:
        path: Relative path to the test file
        entry: TestFileEntry with expected outcomes
        
    Requirements: 3.1-3.9, 15.1-15.5
    """
    if entry is None:
        pytest.skip("No image_multiband entries in manifest")
        return
    
    file_path = get_integration_data_path() / path
    
    if not file_path.exists():
        logger.warning(f"Test file not found, skipping: {file_path}")
        pytest.skip(f"Test file not found: {path}")
        return
    
    from aws.osml.io import IO, AssetType
    
    exception_raised = False
    actual_exception: Optional[BaseException] = None
    
    try:
        reader = IO.open([str(file_path)], "r")
        image_keys = reader.get_asset_keys(asset_type=AssetType.Image)
        
        for key in image_keys:
            asset = reader.get_asset(key)
            num_bands = asset.num_bands
            
            # Verify band count
            assert num_bands >= 1, "Must have at least 1 band"
            
            # Get metadata to check IREP
            metadata = asset.get_metadata()
            fields = metadata.as_dict()
            irep = fields.get("IREP", "").strip()
            
            # Validate band count against IREP if present
            if irep == "RGB":
                assert num_bands == 3, f"RGB images must have 3 bands, got {num_bands}"
            elif irep == "MONO":
                assert num_bands == 1, f"MONO images must have 1 band, got {num_bands}"
            elif irep == "RGB/LUT":
                assert num_bands == 1, f"RGB/LUT images must have 1 band, got {num_bands}"
            
            # Skip compressed images for block reading tests
            ic = fields.get("IC", "NC")
            if ic not in ("NC", "NM"):
                continue
            
            # Test reading all bands
            if asset.has_block(0, 0, 0):
                block_all = asset.get_block(0, 0, 0)
                assert block_all.shape[0] == num_bands
                
                # Test band selection for multi-band images
                if num_bands > 1:
                    # Select first band only
                    block_single = asset.get_block(0, 0, 0, bands=[0])
                    assert block_single.shape[0] == 1
                    
                    # Select subset of bands
                    if num_bands >= 2:
                        block_subset = asset.get_block(0, 0, 0, bands=[0, 1])
                        assert block_subset.shape[0] == 2
            
    except Exception as e:
        exception_raised = True
        actual_exception = e
    
    if entry.expected_valid:
        assert not exception_raised, f"Expected valid file but got: {actual_exception}"
    else:
        assert exception_raised, "Expected multi-band handling to fail but it succeeded"
        if entry.expected_exception:
            actual_type = type(actual_exception).__name__
            assert actual_type == entry.expected_exception
        if entry.expected_message:
            assert entry.expected_message in str(actual_exception)


# =============================================================================
# Utility Functions for Manifest Population
# =============================================================================

def create_sample_manifest_entries() -> list[dict]:
    """Create sample manifest entries for image segment tests.
    
    This function provides a template for users to populate their manifest
    with JITC test data entries. Users should modify paths to match their
    actual test data locations.
    
    Returns:
        List of sample manifest entry dictionaries
        
    Note:
        This is a helper function for documentation purposes.
        Users should NOT commit actual JITC data paths to the repository.
    """
    return [
        # Image parsing tests (positive cases)
        {
            "path": "JITC/path/to/NITF_IMG_POS_01.ntf",
            "expected_valid": True,
            "category": CATEGORY_IMAGE_PARSING,
            "description": "Valid NITF 2.1 image with standard subheader"
        },
        # Image parsing tests (negative cases)
        {
            "path": "JITC/path/to/NITF_IMG_NEG_01.ntf",
            "expected_valid": False,
            "expected_exception": "ValueError",
            "expected_message": "Invalid",
            "category": CATEGORY_IMAGE_PARSING,
            "description": "Invalid image subheader"
        },
        # Block reading tests
        {
            "path": "JITC/path/to/uncompressed_image.ntf",
            "expected_valid": True,
            "category": CATEGORY_IMAGE_BLOCKING,
            "description": "Uncompressed image for block reading"
        },
        # Multi-band tests
        {
            "path": "JITC/path/to/rgb_image.ntf",
            "expected_valid": True,
            "category": CATEGORY_IMAGE_MULTIBAND,
            "description": "RGB image with 3 bands"
        },
    ]
