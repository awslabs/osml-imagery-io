# Property-Based Testing Framework

This document describes the property-based testing (PBT) framework for osml-imagery-io, explaining the rationale, organization, and how to extend it.

## Introduction

Property-based testing validates that code satisfies universal properties across many generated inputs, rather than testing specific examples. For image codecs, this approach is particularly valuable because:

1. **Combinatorial explosion**: Image parameters (dimensions, pixel types, band counts, compression modes, block sizes) create a vast input space impossible to cover with example-based tests
2. **Edge case discovery**: Random generation finds edge cases humans might miss
3. **Regression prevention**: Properties serve as executable specifications that catch regressions
4. **Shrinking**: When tests fail, PBT libraries automatically find minimal failing examples

## Property Categories

The framework organizes properties into three categories:

### Roundtrip Properties

Verify that encoding then decoding preserves data:

- **Property 3: Lossless Roundtrip Preservation** - For lossless compression (IC=NC or COMRAT=N001.0), decoded images must exactly match originals
- **Property 4: Lossy Roundtrip Quality Bounds** - For lossy compression, decoded images must meet quality thresholds (PSNR ≥ 30 dB, SSIM ≥ 0.95)
- **Property 10/11: Idempotent Encoding** - Re-encoding produces consistent results

### Structural Properties

Verify block access and resolution level behavior:

- **Property 5: Block Access Completeness** - All valid block coordinates return data
- **Property 6: Block Reassembly Roundtrip** - Reading all blocks and reassembling equals the original
- **Property 7: Invalid Block Coordinate Error Handling** - Invalid coordinates raise appropriate errors
- **Property 12: Resolution Level Consistency** - Each level has dimensions reduced by 2^N

### API Contract Properties

Verify API behavior and polymorphism:

- **Property 8: Metadata Roundtrip Preservation** - Metadata survives encode/decode cycles
- **Property 20: Dataset Round-Trip Consistency** - Written datasets can be read back equivalently
- **Property 23: Format Auto-Detection** - IO.open() correctly detects formats from extensions

## Quality Thresholds

For lossy compression validation:

| Metric | Threshold | Description |
|--------|-----------|-------------|
| PSNR | ≥ 30 dB | Peak Signal-to-Noise Ratio |
| SSIM | ≥ 0.95 | Structural Similarity Index |

These thresholds ensure lossy compression maintains acceptable visual quality while allowing compression artifacts.

## Test Organization

```
tests/property/
├── __init__.py           # Package marker
├── conftest.py           # Shared fixtures, pytest configuration
├── strategies.py         # Reusable hypothesis strategies
├── quality.py            # PSNR/SSIM calculation utilities
├── test_roundtrip.py     # Properties 3, 4, 10, 11
├── test_block_access.py  # Properties 5, 6, 7, 12
├── test_metadata.py      # Property 8
├── test_strategies.py    # Properties 1, 2, 9 (strategy validation)
├── test_api_contracts.py # API polymorphism tests
└── test_io_contracts.py  # Properties 20, 23
```

## Running Property Tests

```bash
# Run all property tests
pytest -m property tests/property/

# Run only property tests (exclude unit tests)
pytest -m property

# Run only unit tests (exclude property tests)
pytest -m "not property"

# Run specific property test file
pytest tests/property/test_roundtrip.py -v

# Run with more examples (slower but more thorough)
pytest tests/property/ --hypothesis-seed=0
```

## Adding New Properties

### 1. Define the Property

Document the property in the design document with:
- Clear statement of what must hold
- Which requirements it validates
- Expected inputs and outputs

### 2. Create or Extend Strategies

If new input types are needed, add strategies to `tests/property/strategies.py`:

```python
@st.composite
def my_new_strategy(draw) -> MyType:
    """Strategy for generating MyType instances."""
    param1 = draw(st.integers(min_value=1, max_value=100))
    param2 = draw(st.text(min_size=1, max_size=10))
    return MyType(param1, param2)
```

### 3. Write the Property Test

Create a test function with `@given` decorator:

```python
@pytest.mark.property
class TestMyProperty:
    """Property N: Description
    
    **Feature: feature-name, Property N: Property Name**
    **Validates: Requirements X.Y**
    """
    
    @given(data=my_new_strategy())
    @settings(max_examples=100, deadline=None)
    def test_my_property(self, data):
        """For any valid input, the property SHALL hold."""
        # Arrange
        ...
        
        # Act
        result = function_under_test(data)
        
        # Assert
        assert property_holds(result)
```

### 4. Tag with Requirements

Include requirement references in docstrings:
```python
"""**Validates: Requirements 1.2, 3.4**"""
```

## Available Strategies

### Image Generation

- `pixel_types()` - UInt8, UInt16, Int16, Float32
- `image_dimensions(min_size, max_size)` - (rows, cols) tuples
- `band_counts(min_bands, max_bands)` - Integer band counts
- `block_sizes()` - Common block dimensions
- `image_arrays(pixel_type, bands, rows, cols)` - NumPy arrays
- `random_image()` - Complete image with metadata
- `edge_case_images()` - Single-pixel, gradients, max values, etc.
- `realistic_image_for_compression()` - Images suitable for lossy compression testing

### Block Coordinates

- `valid_block_coordinates(rows, cols, block_h, block_w)` - Valid (row, col) pairs
- `invalid_block_coordinates(...)` - Out-of-range coordinates for error testing

### Metadata

- `nitf_field_names()` - Valid NITF field names (uppercase alphanumeric)
- `metadata_values()` - Valid metadata value strings
- `metadata_pairs()` - Dictionaries of key-value pairs

## Hypothesis Configuration

Default settings for I/O-bound tests:

```python
from hypothesis import settings, Phase

pbt_settings = settings(
    max_examples=100,           # Sufficient coverage without excessive runtime
    deadline=None,              # Disable deadline for I/O operations
    phases=[Phase.explicit, Phase.reuse, Phase.generate, Phase.shrink],
)
```

## References

- [PyTorch Vision Issue #3912](https://github.com/pytorch/vision/issues/3912) - Roundtrip testing for image codecs
- [Hypothesis: Canonical Serialization](https://hypothesis.works/articles/canonical-serialization/) - Testing serialization with PBT
- [Hypothesis NumPy Strategies](https://hypothesis.readthedocs.io/en/latest/numpy.html) - Generating NumPy arrays
- [proptest Book](https://proptest-rs.github.io/proptest/intro.html) - Rust property-based testing
