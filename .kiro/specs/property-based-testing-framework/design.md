# Design Document: Property-Based Testing Framework

## Overview

This design describes a comprehensive property-based testing (PBT) framework for osml-imagery-io that systematically validates image codec correctness through generated test cases. The framework operates at two layers:

1. **Python (hypothesis)**: Tests the public API contracts, end-to-end roundtrips, and integration scenarios
2. **Rust (proptest)**: Tests internal invariants, parser correctness, and low-level codec properties

The framework provides reusable strategies for generating synthetic images, block coordinates, and metadata, enabling property tests that cover the combinatorial explosion of image parameters without manual test case enumeration.

### Design Principles

- **Small test images**: Generate images 16x16 to 256x256 for fast test execution
- **Structurally complete**: Generate minimal but valid images that exercise all code paths
- **Edge case coverage**: Include boundary conditions (single-pixel, max values, gradients)
- **Quality metrics for lossy**: Use PSNR/SSIM thresholds instead of exact equality
- **Complementary testing**: Property tests validate universal properties; unit tests validate specific examples

## Architecture

```mermaid
graph TB
    subgraph "Python Layer (hypothesis)"
        strategies[strategies.py<br/>Image/Block/Metadata Strategies]
        conftest[conftest.py<br/>Shared Fixtures]
        roundtrip[test_roundtrip.py<br/>Encode/Decode Properties]
        block[test_block_access.py<br/>Block Retrieval Properties]
        metadata[test_metadata.py<br/>Metadata Preservation]
    end
    
    subgraph "Rust Layer (proptest)"
        rust_strat[Rust Strategies<br/>in test modules]
        rust_props[Property Tests<br/>in #[cfg(test)] blocks]
    end
    
    subgraph "Shared Infrastructure"
        quality[Quality Metrics<br/>PSNR/SSIM calculation]
        fixtures[Test Fixtures<br/>Temp files, cleanup]
    end
    
    strategies --> roundtrip
    strategies --> block
    strategies --> metadata
    conftest --> roundtrip
    conftest --> block
    conftest --> metadata
    quality --> roundtrip
    fixtures --> conftest
```

## Components and Interfaces

### Python Strategy Module (tests/property/strategies.py)

The strategy module provides hypothesis strategies for generating test inputs.

```python
from hypothesis import strategies as st
from hypothesis.extra.numpy import arrays
import numpy as np
from aws.osml.io import PixelType

# Pixel type to numpy dtype mapping
PIXEL_TYPE_DTYPES = {
    PixelType.UInt8: np.uint8,
    PixelType.UInt16: np.uint16,
    PixelType.Int16: np.int16,
    PixelType.Float32: np.float32,
}

def pixel_types() -> st.SearchStrategy[PixelType]:
    """Strategy for supported pixel types."""
    return st.sampled_from([PixelType.UInt8, PixelType.UInt16, PixelType.Int16, PixelType.Float32])

def image_dimensions(min_size: int = 16, max_size: int = 256) -> st.SearchStrategy[tuple[int, int]]:
    """Strategy for image dimensions (width, height)."""
    return st.tuples(
        st.integers(min_value=min_size, max_value=max_size),
        st.integers(min_value=min_size, max_value=max_size)
    )

def band_counts(min_bands: int = 1, max_bands: int = 8) -> st.SearchStrategy[int]:
    """Strategy for number of bands."""
    return st.integers(min_value=min_bands, max_value=max_bands)

def block_sizes() -> st.SearchStrategy[tuple[int, int]]:
    """Strategy for block dimensions."""
    return st.sampled_from([(32, 32), (64, 64), (128, 128), (256, 256)])

def image_arrays(
    pixel_type: PixelType,
    num_bands: int,
    num_rows: int,
    num_cols: int,
) -> st.SearchStrategy[np.ndarray]:
    """Strategy for generating image data arrays in BSQ format (bands, rows, cols)."""
    dtype = PIXEL_TYPE_DTYPES[pixel_type]
    return arrays(dtype=dtype, shape=(num_bands, num_rows, num_cols))

def random_image() -> st.SearchStrategy[tuple[np.ndarray, PixelType, int, int, int]]:
    """Composite strategy for random images with metadata."""
    # Returns (array, pixel_type, num_bands, num_rows, num_cols)
    ...

def edge_case_images() -> st.SearchStrategy[np.ndarray]:
    """Strategy for edge case images: single-pixel, gradients, max values, etc."""
    ...

def valid_block_coordinates(
    num_rows: int,
    num_cols: int,
    block_height: int,
    block_width: int,
) -> st.SearchStrategy[tuple[int, int]]:
    """Strategy for valid block (row, col) coordinates."""
    num_block_rows = (num_rows + block_height - 1) // block_height
    num_block_cols = (num_cols + block_width - 1) // block_width
    return st.tuples(
        st.integers(min_value=0, max_value=num_block_rows - 1),
        st.integers(min_value=0, max_value=num_block_cols - 1)
    )

def nitf_field_names() -> st.SearchStrategy[str]:
    """Strategy for valid NITF field names (uppercase alphanumeric, 1-10 chars)."""
    return st.from_regex(r"[A-Z][A-Z0-9]{0,9}", fullmatch=True)

def metadata_values() -> st.SearchStrategy[str]:
    """Strategy for valid metadata values."""
    return st.text(alphabet=st.characters(whitelist_categories=('L', 'N', 'P')), min_size=1, max_size=20)
```

### Quality Metrics Module (tests/property/quality.py)

Provides PSNR and SSIM calculation for lossy compression validation.

```python
import numpy as np

def calculate_psnr(original: np.ndarray, decoded: np.ndarray) -> float:
    """Calculate Peak Signal-to-Noise Ratio in dB."""
    mse = np.mean((original.astype(np.float64) - decoded.astype(np.float64)) ** 2)
    if mse == 0:
        return float('inf')
    max_pixel = np.iinfo(original.dtype).max if np.issubdtype(original.dtype, np.integer) else 1.0
    return 20 * np.log10(max_pixel / np.sqrt(mse))

def calculate_ssim(original: np.ndarray, decoded: np.ndarray) -> float:
    """Calculate Structural Similarity Index (simplified implementation)."""
    # Uses scikit-image if available, otherwise simplified calculation
    ...

# Quality thresholds
MIN_PSNR_DB = 30.0
MIN_SSIM = 0.95
```

### Shared Fixtures (tests/property/conftest.py)

```python
import pytest
import tempfile
from pathlib import Path

@pytest.fixture
def temp_nitf_path():
    """Fixture providing a temporary NITF file path with cleanup."""
    with tempfile.NamedTemporaryFile(suffix='.ntf', delete=False) as f:
        path = Path(f.name)
    yield path
    if path.exists():
        path.unlink()

@pytest.fixture
def hypothesis_settings():
    """Default hypothesis settings for I/O-bound tests."""
    return {"max_examples": 100, "deadline": None}
```

### Test Module Structure

```
tests/property/
├── __init__.py
├── conftest.py          # Shared fixtures
├── strategies.py        # Image/block/metadata strategies
├── quality.py           # PSNR/SSIM calculation
├── test_roundtrip.py    # Roundtrip properties (Req 2, 3, 6)
├── test_block_access.py # Block access properties (Req 4, 7)
└── test_metadata.py     # Metadata preservation (Req 5)
```

## Data Models

### Image Test Case

```python
@dataclass
class ImageTestCase:
    """Represents a generated test image with its parameters."""
    data: np.ndarray           # Shape: (bands, rows, cols)
    pixel_type: PixelType
    num_bands: int
    num_rows: int
    num_cols: int
    block_width: int
    block_height: int
    
    @property
    def num_block_rows(self) -> int:
        return (self.num_rows + self.block_height - 1) // self.block_height
    
    @property
    def num_block_cols(self) -> int:
        return (self.num_cols + self.block_width - 1) // self.block_width
```

### Compression Configuration

```python
@dataclass
class CompressionConfig:
    """Compression settings for roundtrip tests."""
    ic: str                    # "NC", "C8", "CD"
    comrat: Optional[str]      # "N001.0" for lossless, "01.0" for lossy
    is_lossless: bool
    
    @classmethod
    def uncompressed(cls) -> "CompressionConfig":
        return cls(ic="NC", comrat=None, is_lossless=True)
    
    @classmethod
    def j2k_lossless(cls) -> "CompressionConfig":
        return cls(ic="C8", comrat="N001.0", is_lossless=True)
    
    @classmethod
    def j2k_lossy(cls, bpp: float = 1.0) -> "CompressionConfig":
        return cls(ic="C8", comrat=f"{bpp:04.1f}", is_lossless=False)
```

### Quality Result

```python
@dataclass
class QualityResult:
    """Result of quality comparison between original and decoded images."""
    psnr_db: float
    ssim: float
    shapes_match: bool
    dtypes_match: bool
    
    def meets_lossless_threshold(self) -> bool:
        return self.psnr_db == float('inf') and self.shapes_match and self.dtypes_match
    
    def meets_lossy_threshold(self, min_psnr: float = 30.0, min_ssim: float = 0.95) -> bool:
        return self.psnr_db >= min_psnr and self.ssim >= min_ssim and self.shapes_match and self.dtypes_match
```



## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

The following properties are derived from the requirements acceptance criteria and will be implemented as property-based tests.

### Property 1: Image Strategy Configuration Consistency

*For any* valid image configuration (pixel type, band count, dimensions), the Image_Strategy SHALL produce a NumPy array with shape (bands, rows, cols) matching the configuration and dtype matching the pixel type.

**Validates: Requirements 1.1, 1.2, 1.3, 1.4**

### Property 2: Block Strategy Coordinate Validity

*For any* image dimensions and block dimensions, the Block_Strategy SHALL produce block coordinates (row, col) that are within the valid range [0, num_block_rows) × [0, num_block_cols).

**Validates: Requirements 1.6**

### Property 3: Lossless Roundtrip Preservation

*For any* valid image with lossless compression settings (IC=NC or COMRAT=N001.0), encoding then decoding SHALL produce an image that is exactly equal to the original (same shape, same dtype, same pixel values).

**Validates: Requirements 2.1, 2.2, 2.3, 2.4, 2.5**

### Property 4: Lossy Roundtrip Quality Bounds

*For any* valid image with lossy compression settings, encoding then decoding SHALL produce an image with PSNR >= 30 dB and SSIM >= 0.95, with preserved shape and pixel type.

**Validates: Requirements 3.1, 3.4, 3.5**

### Property 5: Block Access Completeness

*For any* valid block coordinates within an image's block grid, get_block SHALL return a block without error, with shape consistent with the block dimensions (or smaller for edge blocks).

**Validates: Requirements 4.1, 4.2**

### Property 6: Block Reassembly Roundtrip

*For any* image, reading all blocks via get_block and reassembling them in order SHALL produce an array equal to the original image data.

**Validates: Requirements 4.3**

### Property 7: Invalid Block Coordinate Error Handling

*For any* block coordinates outside the valid range, get_block SHALL raise an appropriate error rather than returning invalid data or crashing.

**Validates: Requirements 4.4**

### Property 8: Metadata Roundtrip Preservation

*For any* valid metadata key-value pairs attached to an image, encoding then decoding SHALL preserve all metadata key-value pairs.

**Validates: Requirements 5.1, 5.3**

### Property 9: Metadata Strategy Validity

*For any* generated metadata key-value pair, the key SHALL be a valid NITF field name (uppercase alphanumeric, 1-10 chars) and the value SHALL be a valid string.

**Validates: Requirements 5.2**

### Property 10: Idempotent Encoding (Byte-Level)

*For any* valid image with deterministic codec settings, encode(decode(encode(image))) SHALL produce bytes identical to encode(image).

**Validates: Requirements 6.1**

### Property 11: Idempotent Encoding (Value-Level)

*For any* valid image with lossless compression, decode(encode(decode(encode(image)))) SHALL equal the original image.

**Validates: Requirements 6.2**

### Property 12: Resolution Level Consistency

*For any* image with multiple resolution levels, resolution level N SHALL have dimensions reduced by factor 2^N from level 0, and get_block at level N SHALL return blocks with shapes consistent with that level's dimensions.

**Validates: Requirements 7.1, 7.2, 7.3**

## Error Handling

### Strategy Generation Errors

- **Invalid configuration**: Strategies should reject invalid configurations (e.g., 0 bands, negative dimensions) at strategy construction time
- **Constraint violations**: If hypothesis cannot satisfy constraints after many attempts, it will raise `Unsatisfiable` - strategies should be designed to avoid this

### Test Execution Errors

- **I/O errors**: Temporary file creation/deletion failures should be caught and reported clearly
- **Codec errors**: Encoding/decoding failures should be captured with full context (image parameters, compression settings)
- **Quality threshold failures**: Lossy tests should report actual PSNR/SSIM values when thresholds are not met

### Hypothesis Configuration

```python
from hypothesis import settings, Phase

# Default settings for I/O-bound property tests
pbt_settings = settings(
    max_examples=100,
    deadline=None,  # Disable deadline for I/O operations
    phases=[Phase.explicit, Phase.reuse, Phase.generate, Phase.shrink],
    suppress_health_check=[],
)
```

## Testing Strategy

### Dual Testing Approach

This framework uses both property-based tests and unit tests as complementary approaches:

- **Property tests**: Verify universal properties across many generated inputs (100+ iterations)
- **Unit tests**: Verify specific examples, edge cases, and error conditions

### Property-Based Testing Configuration

- **Library**: hypothesis (Python), proptest (Rust)
- **Iterations**: Minimum 100 examples per property test
- **Deadline**: Disabled for I/O-bound tests
- **Shrinking**: Enabled to find minimal failing examples

### Test Tagging Convention

Each property test must include a comment referencing the design property:

```python
@given(...)
@settings(max_examples=100, deadline=None)
def test_lossless_roundtrip(image_data, pixel_type, compression):
    """Property 3: Lossless Roundtrip Preservation
    
    For any valid image with lossless compression settings, encoding then
    decoding SHALL produce an image exactly equal to the original.
    
    **Feature: property-based-testing-framework, Property 3: Lossless Roundtrip Preservation**
    **Validates: Requirements 2.1, 2.2, 2.3, 2.4, 2.5**
    """
    ...
```

### Test Organization

```
tests/property/
├── conftest.py           # Shared fixtures, hypothesis settings
├── strategies.py         # Reusable strategies
├── quality.py            # PSNR/SSIM calculation
├── test_roundtrip.py     # Properties 3, 4, 10, 11
├── test_block_access.py  # Properties 5, 6, 7, 12
└── test_metadata.py      # Properties 8, 9
```

### Pytest Marker

Property tests are marked for selective execution:

```python
# In conftest.py
def pytest_configure(config):
    config.addinivalue_line("markers", "property: property-based tests")

# In test files
@pytest.mark.property
class TestRoundtripProperties:
    ...
```

Run property tests only:
```bash
pytest -m property tests/property/
```

## References

- [PyTorch Vision Issue #3912](https://github.com/pytorch/vision/issues/3912): Roundtrip testing for image codecs
- [Hypothesis: Canonical Serialization](https://hypothesis.works/articles/canonical-serialization/): Testing serialization with property-based testing
- [Hypothesis NumPy Strategies](https://hypothesis.readthedocs.io/en/latest/numpy.html): Generating NumPy arrays with hypothesis
- [proptest Book](https://proptest-rs.github.io/proptest/intro.html): Rust property-based testing
