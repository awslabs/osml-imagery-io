# Design Document: API Design Alignment

## Overview

This design document describes the changes needed to align the API design document (`docs/API_DESIGN.md`) with the actual implementation and vice versa. The changes fall into three categories:

1. **Documentation updates**: Updating API_DESIGN.md to reflect actual implementation patterns (property accessors, class signatures)
2. **Implementation updates**: Modifying Python bindings to match documented API (list of URIs, property accessors)
3. **Cleanup**: Removing deprecated concepts (fsspec, FileDatasetReader/Writer) and adding missing documentation (SimpleMetadataProvider, PyStructure classes)

## Architecture

The changes maintain the existing layered architecture:

```
┌─────────────────────────────────────────────────────────────┐
│                    Python API Layer                          │
│  (IO, DatasetReader, DatasetWriter, AssetProviders)         │
├─────────────────────────────────────────────────────────────┤
│                  PyO3 Bindings Layer                         │
│  (src/bindings/*.rs - wraps Rust traits for Python)         │
├─────────────────────────────────────────────────────────────┤
│                    Rust Traits Layer                         │
│  (src/traits/*.rs - abstract interfaces)                    │
├─────────────────────────────────────────────────────────────┤
│              Format-Specific Implementations                 │
│  (src/jbp/*.rs - NITF/NSIF implementation)                  │
└─────────────────────────────────────────────────────────────┘
```

No architectural changes are required. All modifications are at the API surface level.

## Components and Interfaces

### Component 1: IO Class (src/bindings/io.rs)

**Current State:**
```rust
#[staticmethod]
#[pyo3(signature = (uri, mode="r", format=None))]
fn open(py: Python<'_>, uri: &str, mode: &str, format: Option<&str>) -> PyResult<PyObject>
```

**Target State:**
```rust
#[staticmethod]
#[pyo3(signature = (paths, mode="r", format=None))]
fn open(py: Python<'_>, paths: Vec<String>, mode: &str, format: Option<&str>) -> PyResult<PyObject>
```

**Changes:**
- Accept `paths: Vec<String>` instead of `uri: &str`
- Use `paths[0]` for current single-file implementations
- Update API_DESIGN.md to remove fsspec parameter and add format parameter

### Component 2: PyDatasetReader (src/bindings/reader.rs)

**Current State:**
```rust
fn get_metadata(&self) -> PyResult<PyMetadataProvider>
```

**Target State:**
```rust
#[getter]
fn metadata(&self) -> PyResult<PyMetadataProvider>
```

**Changes:**
- Convert `get_metadata()` method to `metadata` property getter
- Update API_DESIGN.md class diagram

### Component 3: PyDatasetWriter (src/bindings/writer.rs)

**Current State:**
- Has both `add_asset()` and `add_image_asset()` methods
- Has `set_metadata()` method

**Target State:**
- Only `add_asset()` method (accepts any AssetProvider including MemoryImageAssetProvider)
- `metadata` property setter

**Changes:**
- Remove `add_image_asset()` method
- Convert `set_metadata()` to property setter
- Update API_DESIGN.md class diagram

### Component 4: PyTextAssetProvider (src/bindings/text.rs)

**Current State:**
```rust
fn get_text(&self) -> PyResult<String>
```

**Target State:**
```rust
#[getter]
fn text(&self) -> PyResult<String>
```

**Changes:**
- Convert `get_text()` method to `text` property getter
- Update API_DESIGN.md class diagram

### Component 5: PyDataAssetProvider (src/bindings/data.rs)

**Current State:**
```rust
fn get_mime_type(&self) -> &str
fn parse_as_xml(&self) -> PyResult<String>  // Returns string
```

**Target State:**
```rust
#[getter]
fn mime_type(&self) -> &str
fn parse_as_xml(&self, py: Python<'_>) -> PyResult<PyObject>  // Returns ElementTree
```

**Changes:**
- Convert `get_mime_type()` to `mime_type` property getter
- Modify `parse_as_xml()` to return Python ElementTree object
- Use Python's xml.etree.ElementTree for parsing (safe by default for trusted input)
- Update API_DESIGN.md class diagram

### Component 6: API_DESIGN.md Updates

**Sections to Update:**

1. **Core API Structure diagram**: 
   - IO class: `open(paths: List[str], mode: str, format: Optional[str])`
   - DatasetReader: `metadata` property instead of `get_metadata()`
   - DatasetWriter: `metadata` property instead of `set_metadata()`

2. **Asset Provider Hierarchy diagram**:
   - All `get_*()` methods become properties
   - Add `from_bytes()` static method

3. **ImageAssetProvider Hierarchy diagram**:
   - `get_block()` bands parameter shown as optional
   - Update MemoryImageAssetProvider constructor to `create()` static method

4. **Format-Specific Implementations diagram**:
   - Remove FilesDatasetReader/Writer
   - Remove fsspec references from JBPDatasetReader/Writer

5. **Usage Examples**:
   - Update all examples to use property accessors
   - Update import paths and class names
   - Add SimpleMetadataProvider examples
   - Add PyStructure classes examples

**New Sections to Add:**

1. **SimpleMetadataProvider**: Document the mutable metadata provider
2. **Parser Infrastructure (PyStructure Classes)**: Document the binary parser classes

## Data Models

No changes to data models. The existing types remain:

- `AssetType` enum: Image, Text, Graphics, Data
- `PixelType` enum: UInt8, UInt16, Int16, Float32, etc.
- `MetadataProvider` trait: raw bytes and as_dict() access
- `Value` enum: String, Bytes, Unsigned, Array, Struct (for parser)

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: IO.open paths list handling

*For any* non-empty list of valid URI strings, calling `IO.open(paths, mode)` SHALL successfully open the dataset using the first path in the list, regardless of how many paths are provided.

**Validates: Requirements 1.2, 1.3**

### Property 2: DatasetReader metadata property accessor

*For any* valid DatasetReader instance obtained from IO.open(), accessing `reader.metadata` SHALL return a valid MetadataProvider object with accessible `raw` and `as_dict()` methods.

**Validates: Requirements 2.2**

### Property 3: add_asset accepts all AssetProvider subtypes

*For any* AssetProvider subtype (including MemoryImageAssetProvider, BytesAssetProvider), calling `writer.add_asset(key, provider, title, description, roles)` SHALL successfully add the asset to the dataset without requiring a separate method.

**Validates: Requirements 3.2**

### Property 4: TextAssetProvider text property accessor

*For any* valid TextAssetProvider instance, accessing `provider.text` SHALL return a string containing the decoded text content.

**Validates: Requirements 6.4**

### Property 5: DataAssetProvider mime_type property accessor

*For any* valid DataAssetProvider instance, accessing `provider.mime_type` SHALL return a string containing the MIME type of the data.

**Validates: Requirements 7.2**

### Property 6: XML parsing returns traversable ElementTree

*For any* DataAssetProvider containing valid XML content, calling `parse_as_xml()` SHALL return a Python ElementTree Element object that supports standard ElementTree traversal methods (find, findall, iter, attrib, text).

**Validates: Requirements 7.3**

## Error Handling

Existing error handling patterns are preserved:

- `CodecError::AssetNotFound` - When asset key doesn't exist
- `CodecError::InvalidFormat` - When file format is unsupported
- `CodecError::Parse` - When XML/JSON parsing fails
- `CodecError::Io` - For I/O errors

New error cases:
- `IO.open([])` with empty list - Should raise `ValueError` with message "paths list cannot be empty"

## Testing Strategy

### Unit Tests

Unit tests will verify specific examples and edge cases:

1. **IO.open with list parameter**:
   - Test with single-element list `["file.ntf"]`
   - Test with multi-element list `["file1.ntf", "file2.ntf"]` (verify first is used)
   - Test with empty list `[]` (verify ValueError raised)

2. **Property accessor tests**:
   - Test `reader.metadata` returns MetadataProvider
   - Test `provider.text` returns string for TextAssetProvider
   - Test `provider.mime_type` returns string for DataAssetProvider

3. **add_asset with MemoryImageAssetProvider**:
   - Test adding MemoryImageAssetProvider via add_asset
   - Verify asset is retrievable after write/close/reopen

4. **XML parsing**:
   - Test parse_as_xml returns ElementTree Element
   - Test Element.find(), Element.findall(), Element.iter() work
   - Test invalid XML raises CodecError::Parse

5. **Removed methods**:
   - Test that `add_image_asset` no longer exists on PyDatasetWriter
   - Test that `get_metadata` no longer exists (replaced by property)

### Property-Based Tests

Property-based tests will use `hypothesis` (Python) to verify universal properties:

**Configuration:**
- Minimum 100 iterations per property test
- Tag format: `Feature: api-design-alignment, Property N: description`

**Property Tests:**

1. **Property 1: IO.open paths list handling**
   - Generate random non-empty lists of valid file paths
   - Verify IO.open succeeds and uses first path
   - Strategy: `lists(text(min_size=1), min_size=1)` for path generation

2. **Property 2: DatasetReader metadata property**
   - For any valid NITF file, reader.metadata returns MetadataProvider
   - MetadataProvider.as_dict() returns dict
   - MetadataProvider.raw returns bytes-like object

3. **Property 3: add_asset type hierarchy**
   - Generate random AssetProvider subtypes (MemoryImageAssetProvider, BytesAssetProvider)
   - Verify add_asset accepts all without error

4. **Property 4: TextAssetProvider text property**
   - For any TextAssetProvider, text property returns string
   - String is non-None

5. **Property 5: DataAssetProvider mime_type property**
   - For any DataAssetProvider, mime_type property returns string
   - String is non-empty

6. **Property 6: XML parsing returns ElementTree**
   - Generate random valid XML strings
   - Verify parse_as_xml returns Element with traversal methods
   - Strategy: Use hypothesis-xml or custom XML generator

### Integration Tests

Integration tests will verify end-to-end workflows:

1. **Read workflow with new API**:
   - Open NITF file with `IO.open(["file.ntf"], "r")`
   - Access `reader.metadata` property
   - Get image asset and verify properties work

2. **Write workflow with unified add_asset**:
   - Create MemoryImageAssetProvider
   - Add via `writer.add_asset()` (not add_image_asset)
   - Close, reopen, verify asset readable

3. **XML data asset workflow**:
   - Open file with XML data asset
   - Call `parse_as_xml()`
   - Traverse returned ElementTree
