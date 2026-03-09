# Functional Test Scenarios: Dataset Writer Encoding Hints

This document describes functional test scenarios for the Dataset Writer Encoding Hints feature. These tests validate end-to-end workflows that span multiple components.

> **Note**: These scenarios are documented for future implementation. The user will update this file as tests are implemented.

## Test Scenario 1: Read-Modify-Write Workflow

### Description

Validates that encoding hints can be read from an existing NITF file, modified, and written to a new file with the modified hints applied.

### Preconditions

- A valid NITF file exists with known encoding parameters (imode, nppbh, nppbv)
- JBPDatasetReader can successfully read the file
- JBPDatasetWriter is available

### Test Steps

1. **Read source file**
   ```python
   reader = JBPDatasetReader(source_path)
   dataset = reader.open()
   image_asset = dataset.assets()[0]
   original_metadata = image_asset.metadata().as_dict()
   ```

2. **Copy metadata to SimpleMetadataProvider**
   ```python
   metadata = SimpleMetadataProvider(image_asset.metadata())
   ```

3. **Modify encoding hints**
   ```python
   metadata.set("imode", "P")  # Change from original
   metadata.set("nppbh", "128")
   metadata.set("nppbv", "128")
   ```

4. **Create new asset with modified metadata**
   ```python
   new_asset = MemoryImageAssetProvider.create(
       key="modified_image",
       num_columns=image_asset.num_columns(),
       num_rows=image_asset.num_rows(),
       num_bands=image_asset.num_bands(),
       pixel_type=image_asset.pixel_type(),
       metadata=metadata
   )
   # Copy pixel data from original to new asset
   ```

5. **Write new file**
   ```python
   writer = JBPDatasetWriter(output_path)
   writer.add_image_asset(new_asset)
   writer.write()
   ```

6. **Read back and verify**
   ```python
   verify_reader = JBPDatasetReader(output_path)
   verify_dataset = verify_reader.open()
   verify_metadata = verify_dataset.assets()[0].metadata().as_dict()
   ```

### Expected Behaviors

| Aspect | Expected Result |
|--------|-----------------|
| imode field | Should be "P" (modified value) |
| nppbh field | Should be "128" (modified value) |
| nppbv field | Should be "128" (modified value) |
| Pixel data | Should match original image |
| Other metadata | Should be preserved from original |

### Validation Criteria

- [ ] Modified encoding hints appear in output file
- [ ] Field names are consistent (lowercase) between read and write
- [ ] No data corruption in pixel values
- [ ] File is valid NITF that can be opened by other readers

---

## Test Scenario 2: Synthetic Image with Encoding Hints

### Description

Validates that a synthetic image created with MemoryImageAssetProvider correctly applies encoding hints when written to a NITF file.

### Preconditions

- SimpleMetadataProvider is available
- MemoryImageAssetProvider is available
- JBPDatasetWriter is available

### Test Steps

1. **Create metadata with encoding hints**
   ```python
   metadata = SimpleMetadataProvider()
   metadata.set("imode", "R")  # Row interleave
   metadata.set("nppbh", "256")
   metadata.set("nppbv", "256")
   metadata.set("ic", "NC")  # No compression
   ```

2. **Create synthetic image asset**
   ```python
   asset = MemoryImageAssetProvider.create(
       key="synthetic_image",
       num_columns=512,
       num_rows=512,
       num_bands=3,
       block_width=256,
       block_height=256,
       pixel_type=PixelType.UInt8,
       metadata=metadata
   )
   ```

3. **Generate test pattern data**
   ```python
   # Fill with gradient or checkerboard pattern for visual verification
   for band in range(3):
       data = generate_test_pattern(512, 512, band)
       asset.set_band_data(band, data)
   ```

4. **Write to NITF file**
   ```python
   writer = JBPDatasetWriter(output_path)
   writer.add_image_asset(asset)
   writer.write()
   ```

5. **Read back and verify**
   ```python
   reader = JBPDatasetReader(output_path)
   dataset = reader.open()
   result_asset = dataset.assets()[0]
   result_metadata = result_asset.metadata().as_dict()
   ```

### Expected Behaviors

| Aspect | Expected Result |
|--------|-----------------|
| imode field | Should be "R" |
| nppbh field | Should be "256" |
| nppbv field | Should be "256" |
| ic field | Should be "NC" |
| Image dimensions | 512 x 512 |
| Band count | 3 |
| Pixel values | Match generated test pattern |

### Validation Criteria

- [ ] All encoding hints are correctly written to file
- [ ] Image subheader contains expected field values
- [ ] Pixel data round-trips without corruption
- [ ] Block structure matches specified nppbh/nppbv

---

## Test Scenario 3: Encoding Hint Validation Errors

### Description

Validates that invalid encoding hints produce appropriate error messages.

### Test Cases

#### 3.1 Invalid IMODE Value

```python
metadata = SimpleMetadataProvider()
metadata.set("imode", "X")  # Invalid value

asset = MemoryImageAssetProvider.create(
    key="test", num_columns=64, num_rows=64, metadata=metadata
)

writer = JBPDatasetWriter(output_path)
writer.add_image_asset(asset)

# Expected: Error with message containing "Invalid imode value 'X'"
with pytest.raises(Exception) as exc_info:
    writer.write()
assert "imode" in str(exc_info.value).lower()
```

#### 3.2 Invalid Block Size (Out of Range)

```python
metadata = SimpleMetadataProvider()
metadata.set("nppbh", "0")  # Invalid: must be >= 1

# Expected: Error with message about invalid nppbh value
```

```python
metadata = SimpleMetadataProvider()
metadata.set("nppbv", "10000")  # Invalid: must be <= 8192

# Expected: Error with message about invalid nppbv value
```

### Expected Behaviors

| Invalid Input | Expected Error |
|---------------|----------------|
| imode = "X" | "Invalid imode value 'X': must be B, P, R, or S" |
| nppbh = "0" | "Invalid nppbh value '0': must be between 1 and 8192" |
| nppbv = "10000" | "Invalid nppbv value '10000': must be between 1 and 8192" |

---

## Test Scenario 4: Block Size Auto-Adjustment

### Description

Validates that block sizes larger than image dimensions are automatically adjusted.

### Test Steps

1. **Create metadata with oversized block hints**
   ```python
   metadata = SimpleMetadataProvider()
   metadata.set("nppbh", "1024")  # Larger than image width
   metadata.set("nppbv", "1024")  # Larger than image height
   ```

2. **Create small image**
   ```python
   asset = MemoryImageAssetProvider.create(
       key="small_image",
       num_columns=64,  # Smaller than nppbh
       num_rows=64,     # Smaller than nppbv
       metadata=metadata
   )
   ```

3. **Write and read back**
   ```python
   writer = JBPDatasetWriter(output_path)
   writer.add_image_asset(asset)
   writer.write()  # Should succeed with warning
   
   reader = JBPDatasetReader(output_path)
   dataset = reader.open()
   result_metadata = dataset.assets()[0].metadata().as_dict()
   ```

### Expected Behaviors

| Aspect | Expected Result |
|--------|-----------------|
| Write operation | Succeeds (no error) |
| Warning logged | "nppbh 1024 exceeds image width 64, adjusting to 64" |
| Actual nppbh | 64 (clamped to image width) |
| Actual nppbv | 64 (clamped to image height) |

### Validation Criteria

- [ ] Write succeeds without error
- [ ] Warning is logged about adjustment
- [ ] Output file has block sizes clamped to image dimensions

---

## Test Scenario 5: Provider Properties Override Metadata

### Description

Validates that structural properties from the provider take precedence over conflicting metadata values.

### Test Steps

1. **Create metadata with conflicting structural values**
   ```python
   metadata = SimpleMetadataProvider()
   metadata.set("nbands", "5")  # Conflicts with provider
   metadata.set("nrows", "1000")  # Conflicts with provider
   metadata.set("ncols", "1000")  # Conflicts with provider
   ```

2. **Create asset with different structural properties**
   ```python
   asset = MemoryImageAssetProvider.create(
       key="conflict_test",
       num_columns=256,  # Different from metadata
       num_rows=256,     # Different from metadata
       num_bands=3,      # Different from metadata
       metadata=metadata
   )
   ```

3. **Write and verify**
   ```python
   writer = JBPDatasetWriter(output_path)
   writer.add_image_asset(asset)
   writer.write()
   
   reader = JBPDatasetReader(output_path)
   dataset = reader.open()
   result_asset = dataset.assets()[0]
   ```

### Expected Behaviors

| Property | Metadata Value | Provider Value | Output Value |
|----------|----------------|----------------|--------------|
| nbands | 5 | 3 | 3 (provider wins) |
| nrows | 1000 | 256 | 256 (provider wins) |
| ncols | 1000 | 256 | 256 (provider wins) |

### Validation Criteria

- [ ] Provider structural properties are used in output
- [ ] Warning logged about conflicts
- [ ] File is valid and readable

---

## Implementation Notes

### Test Data Requirements

- Sample NITF files with various imode values (B, P, R, S)
- Files with different block sizes for round-trip testing
- Multi-band imagery for imode testing

### Test Infrastructure

- Tests should use temporary directories for output files
- Cleanup should remove generated files after test completion
- Consider using pytest fixtures for common setup/teardown

### Coverage Goals

- All valid imode values (B, P, R, S)
- Block size edge cases (1, 8192, image dimensions)
- Multi-band images (1, 3, 4+ bands)
- Various pixel types (UInt8, UInt16, Float32)
