# Requirements Document

## Introduction

This feature adds GeoTIFF metadata parsing and writing to the osml-imagery-io library. GeoTIFF extends TIFF with geospatial metadata stored in three special tags: a GeoKey directory (tag 34735), double parameters (tag 34736), and ASCII parameters (tag 34737), plus transformation tags for pixel-to-CRS mapping (ModelTiepointTag, ModelPixelScaleTag, ModelTransformationTag). The implementation parses these tags in pure Rust (no libgeotiff dependency), maps GeoKeys to human-readable metadata fields, and exposes everything through the existing `MetadataProvider` interface. GeoTransform computation (deriving GDAL-convention affine transforms from the raw tags) is out of scope for the IO module and belongs in a separate library. Writer support allows setting GeoTIFF tags from encoding hints in `BufferedMetadataProvider`. This is Phase 3 of the TIFF roadmap, building on the Phase 1 (libtiff FFI + reading) and Phase 2 (writing) already implemented.

## Glossary

- **GeoKey_Directory**: The GeoKeyDirectoryTag (TIFF tag 34735), a `SHORT` array containing a header (KeyDirectoryVersion, KeyRevision, MinorRevision, NumberOfKeys) followed by key entries. Each key entry is four `SHORT` values: KeyID, TIFFTagLocation, Count, Value_Offset.
- **GeoDouble_Params**: The GeoDoubleParamsTag (TIFF tag 34736), a `DOUBLE` array containing floating-point parameter values referenced by GeoKey entries with TIFFTagLocation=34736.
- **GeoASCII_Params**: The GeoAsciiParamsTag (TIFF tag 34737), an ASCII string containing text parameter values referenced by GeoKey entries with TIFFTagLocation=34737. Individual strings are pipe-delimited (`|`).
- **GeoKey**: A single entry in the GeoKey directory, identified by a KeyID (e.g., 1024 for GTModelTypeGeoKey). The value is either inline (TIFFTagLocation=0, value in Value_Offset field) or referenced from GeoDouble_Params or GeoASCII_Params.
- **ModelTiepoint_Tag**: TIFF tag 33922, a `DOUBLE` array of tiepoint tuples `(pixel_x, pixel_y, pixel_z, geo_x, geo_y, geo_z)` mapping pixel coordinates to CRS coordinates.
- **ModelPixelScale_Tag**: TIFF tag 33550, a `DOUBLE` array of three values `(scale_x, scale_y, scale_z)` representing pixel size in CRS units.
- **ModelTransformation_Tag**: TIFF tag 34264, a `DOUBLE` array of 16 values representing a 4×4 affine transformation matrix from pixel coordinates to CRS coordinates.
- **TIFFMetadataProvider**: The existing TIFF metadata provider that maps TIFF tags to key-value pairs. Extended in this feature to include GeoTIFF metadata fields.
- **TIFFDatasetReader**: The existing TIFF dataset reader implementing `DatasetReader`. Extended to parse GeoTIFF tags during IFD enumeration.
- **TIFFDatasetWriter**: The existing TIFF dataset writer implementing `DatasetWriter`. Extended to write GeoTIFF tags from encoding hints.
- **TiffHandle**: The safe RAII wrapper around libtiff providing typed tag access. Extended with array tag getters for `u16[]` and `f64[]`.
- **MetadataProvider**: The trait providing metadata as key-value dictionaries, defined in `src/traits/metadata.rs`.
- **BufferedMetadataProvider**: The in-memory `MetadataProvider` used to supply encoding hints to the writer.
- **EPSG_Code**: A numeric identifier from the EPSG Geodetic Parameter Dataset identifying a coordinate reference system (e.g., 32618 for UTM Zone 18N).
- **GeoTIFF_Parser**: The `src/tiff/geotiff.rs` module responsible for parsing GeoKey directory, double/ASCII params, and transformation tags into structured metadata.

## Requirements

### Requirement 1: FFI Extensions for Array Tag Access

**User Story:** As a developer, I want the TiffHandle to support reading and writing array-valued TIFF tags (u16 arrays and f64 arrays), so that GeoTIFF tags can be accessed through the existing safe FFI wrapper.

#### Acceptance Criteria

1. THE TiffHandle SHALL provide a `get_field_u16_array(tag, count) -> Result<Vec<u16>>` method that reads a `SHORT` array tag of the specified count from the current IFD via `TIFFGetField`.
2. THE TiffHandle SHALL provide a `get_field_f64_array(tag, count) -> Result<Vec<f64>>` method that reads a `DOUBLE` array tag of the specified count from the current IFD via `TIFFGetField`.
3. THE TiffHandle SHALL provide a `set_field_u16_array(tag, data: &[u16]) -> Result<()>` method that writes a `SHORT` array tag to the current IFD via `TIFFSetField`.
4. THE TiffHandle SHALL provide a `set_field_f64_array(tag, data: &[f64]) -> Result<()>` method that writes a `DOUBLE` array tag to the current IFD via `TIFFSetField`.
5. THE TiffHandle SHALL provide a `set_field_string(tag, value: &str) -> Result<()>` method that writes an ASCII string tag to the current IFD via `TIFFSetField`.
6. IF a requested array tag is not present in the current IFD, THEN THE TiffHandle SHALL return a `CodecError::Decode` error with a message identifying the missing tag.

### Requirement 2: GeoTIFF Tag Constants

**User Story:** As a developer, I want named constants for GeoTIFF-specific TIFF tags and GeoKey IDs, so that the GeoTIFF parser uses readable identifiers instead of magic numbers.

#### Acceptance Criteria

1. THE tags_module SHALL define constants for GeoTIFF TIFF tags: `GEO_KEY_DIRECTORY_TAG` (34735), `GEO_DOUBLE_PARAMS_TAG` (34736), `GEO_ASCII_PARAMS_TAG` (34737), `MODEL_TIEPOINT_TAG` (33922), `MODEL_PIXEL_SCALE_TAG` (33550), and `MODEL_TRANSFORMATION_TAG` (34264).
2. THE tags_module SHALL define constants for GeoKey IDs: `GT_MODEL_TYPE_GEO_KEY` (1024), `GT_RASTER_TYPE_GEO_KEY` (1025), `GEOGRAPHIC_TYPE_GEO_KEY` (2048), and `PROJECTED_CS_TYPE_GEO_KEY` (3072).
3. THE tags_module SHALL define constants for GTModelTypeGeoKey values: `MODEL_TYPE_PROJECTED` (1) and `MODEL_TYPE_GEOGRAPHIC` (2).
4. THE tags_module SHALL define constants for GTRasterTypeGeoKey values: `RASTER_PIXEL_IS_AREA` (1) and `RASTER_PIXEL_IS_POINT` (2).

### Requirement 3: GeoKey Directory Parsing

**User Story:** As a developer, I want the GeoTIFF parser to decode the GeoKey directory from tag 34735, so that individual GeoKeys can be resolved to their values.

#### Acceptance Criteria

1. WHEN tag 34735 (GeoKeyDirectoryTag) is present in an IFD, THE GeoTIFF_Parser SHALL read the `SHORT` array and parse the 4-value header: KeyDirectoryVersion, KeyRevision, MinorRevision, NumberOfKeys.
2. WHEN tag 34735 is present, THE GeoTIFF_Parser SHALL parse each key entry (KeyID, TIFFTagLocation, Count, Value_Offset) from the directory following the header.
3. WHEN a GeoKey entry has TIFFTagLocation=0, THE GeoTIFF_Parser SHALL interpret the Value_Offset field as the inline `SHORT` value of the key.
4. WHEN a GeoKey entry has TIFFTagLocation=34736, THE GeoTIFF_Parser SHALL read the corresponding `DOUBLE` value(s) from the GeoDouble_Params array at the offset and count specified by Value_Offset and Count.
5. WHEN a GeoKey entry has TIFFTagLocation=34737, THE GeoTIFF_Parser SHALL read the corresponding ASCII string from the GeoASCII_Params string at the offset and count specified by Value_Offset and Count, stripping the trailing pipe delimiter (`|`).
6. IF tag 34735 is not present in the IFD, THEN THE GeoTIFF_Parser SHALL produce no GeoKey metadata fields (the file is a plain TIFF, not a GeoTIFF).
7. IF the GeoKey directory array length is less than 4 (insufficient for the header), THEN THE GeoTIFF_Parser SHALL return a `CodecError::Decode` error describing the malformed directory.
8. IF a GeoKey entry references GeoDouble_Params (tag 34736) but that tag is absent, THEN THE GeoTIFF_Parser SHALL return a `CodecError::Decode` error identifying the missing parameter tag.
9. IF a GeoKey entry references GeoASCII_Params (tag 34737) but that tag is absent, THEN THE GeoTIFF_Parser SHALL return a `CodecError::Decode` error identifying the missing parameter tag.

### Requirement 4: GeoKey to Metadata Mapping

**User Story:** As a developer, I want GeoKeys mapped to human-readable metadata fields, so that I can inspect coordinate reference system information without decoding raw GeoKey IDs.

#### Acceptance Criteria

1. WHEN GTModelTypeGeoKey (1024) has value 1, THE GeoTIFF_Parser SHALL produce metadata field `"GeoModelType"` with string value `"Projected"`.
2. WHEN GTModelTypeGeoKey (1024) has value 2, THE GeoTIFF_Parser SHALL produce metadata field `"GeoModelType"` with string value `"Geographic"`.
3. WHEN GTRasterTypeGeoKey (1025) has value 1, THE GeoTIFF_Parser SHALL produce metadata field `"GeoRasterType"` with string value `"PixelIsArea"`.
4. WHEN GTRasterTypeGeoKey (1025) has value 2, THE GeoTIFF_Parser SHALL produce metadata field `"GeoRasterType"` with string value `"PixelIsPoint"`.
5. WHEN ProjectedCSTypeGeoKey (3072) is present, THE GeoTIFF_Parser SHALL produce metadata field `"GeoProjectedCRS"` with the EPSG_Code as a JSON number value (e.g., `32618`).
6. WHEN GeographicTypeGeoKey (2048) is present, THE GeoTIFF_Parser SHALL produce metadata field `"GeoGeographicCRS"` with the EPSG_Code as a JSON number value (e.g., `4326`).
7. WHEN a GeoKey ID is not one of the explicitly mapped keys (1024, 1025, 2048, 3072), THE GeoTIFF_Parser SHALL produce a metadata field with key `"GeoKey_{KeyID}"` and the raw value using the most appropriate JSON type: a JSON number for inline SHORT and DOUBLE values, or a JSON string for ASCII values.

### Requirement 5: Transformation Tag Parsing

**User Story:** As a developer, I want ModelTiepointTag, ModelPixelScaleTag, and ModelTransformationTag parsed into structured metadata fields, so that I can compute pixel-to-CRS coordinate transformations.

#### Acceptance Criteria

1. WHEN ModelPixelScaleTag (33550) is present, THE GeoTIFF_Parser SHALL read the 3-element `DOUBLE` array and produce metadata field `"GeoPixelScale"` with a `serde_json::Value::Array` of 3 JSON number values `[scale_x, scale_y, scale_z]`.
2. WHEN ModelTiepointTag (33922) is present, THE GeoTIFF_Parser SHALL read the `DOUBLE` array and produce metadata field `"GeoTiepoints"` with a `serde_json::Value::Array` of 6-element arrays: `[[pixel_x, pixel_y, pixel_z, geo_x, geo_y, geo_z], ...]`.
3. WHEN ModelTransformationTag (34264) is present, THE GeoTIFF_Parser SHALL read the 16-element `DOUBLE` array and produce metadata field `"GeoTransformation"` with a `serde_json::Value::Array` of 16 JSON number values.
4. IF ModelTiepointTag contains a number of values that is not a multiple of 6, THEN THE GeoTIFF_Parser SHALL return a `CodecError::Decode` error describing the malformed tiepoint data.

### Requirement 6: GeoTIFF Metadata Integration with TIFFMetadataProvider

**User Story:** As a developer, I want GeoTIFF metadata fields available alongside standard TIFF tags in the per-IFD MetadataProvider, so that I can access both image properties and geospatial metadata through a single interface.

#### Acceptance Criteria

1. WHEN a TIFF IFD contains GeoTIFF tags, THE TIFFMetadataProvider SHALL include both standard TIFF tag metadata and GeoTIFF metadata fields in the same dictionary returned by `as_dict(None)`.
2. THE TIFFMetadataProvider SHALL prefix all GeoTIFF metadata field keys with `"Geo"` (e.g., `"GeoModelType"`, `"GeoPixelScale"`, `"GeoKey_1024"`), so that `as_dict` section filtering by key prefix can distinguish GeoTIFF fields from standard TIFF fields.
3. WHEN a TIFF IFD does not contain GeoTIFF tags (tag 34735 absent), THE TIFFMetadataProvider SHALL contain only standard TIFF tag metadata with no `"Geo"`-prefixed keys.
4. THE GeoTIFF metadata field values SHALL use native `serde_json::Value` types appropriate to the data: JSON strings for human-readable labels (e.g., `"Projected"`), JSON numbers for numeric identifiers (e.g., EPSG codes), and JSON arrays of numbers for coordinate/transformation data (e.g., pixel scale, tiepoints).

### Requirement 7: GeoTIFF Tag Writing

**User Story:** As a developer, I want to write GeoTIFF tags from encoding hints in BufferedMetadataProvider, so that I can produce georeferenced TIFF files through the DatasetWriter interface.

#### Acceptance Criteria

1. WHEN the dataset-level MetadataProvider contains a `"GeoModelType"` key with value `"Projected"` or `"Geographic"`, THE TIFFDatasetWriter SHALL write the corresponding GTModelTypeGeoKey (1024) in the GeoKey directory.
2. WHEN the dataset-level MetadataProvider contains a `"GeoRasterType"` key with value `"PixelIsArea"` or `"PixelIsPoint"`, THE TIFFDatasetWriter SHALL write the corresponding GTRasterTypeGeoKey (1025) in the GeoKey directory.
3. WHEN the dataset-level MetadataProvider contains a `"GeoProjectedCRS"` key with a JSON number EPSG code, THE TIFFDatasetWriter SHALL write ProjectedCSTypeGeoKey (3072) with that integer value in the GeoKey directory.
4. WHEN the dataset-level MetadataProvider contains a `"GeoGeographicCRS"` key with a JSON number EPSG code, THE TIFFDatasetWriter SHALL write GeographicTypeGeoKey (2048) with that integer value in the GeoKey directory.
5. WHEN the dataset-level MetadataProvider contains a `"GeoPixelScale"` key with a JSON array of 3 numbers, THE TIFFDatasetWriter SHALL write ModelPixelScaleTag (33550) with the values as doubles.
6. WHEN the dataset-level MetadataProvider contains a `"GeoTiepoints"` key with a JSON array of 6-element arrays, THE TIFFDatasetWriter SHALL write ModelTiepointTag (33922) with the flattened double array.
7. WHEN the dataset-level MetadataProvider contains a `"GeoTransformation"` key with a JSON array of 16 numbers, THE TIFFDatasetWriter SHALL write ModelTransformationTag (34264) with the values as doubles.
8. WHEN GeoKeys are present in the encoding hints, THE TIFFDatasetWriter SHALL assemble a valid GeoKey directory (tag 34735) with the correct header (KeyDirectoryVersion=1, KeyRevision=1, MinorRevision=1, NumberOfKeys) and write it along with any required GeoDouble_Params (tag 34736) and GeoASCII_Params (tag 34737) arrays.
9. WHEN no GeoTIFF encoding hints are present in the MetadataProvider, THE TIFFDatasetWriter SHALL not write any GeoTIFF tags (producing a plain TIFF).
10. IF a `"GeoProjectedCRS"` or `"GeoGeographicCRS"` value is not a valid JSON number convertible to a u16 integer, THEN THE TIFFDatasetWriter SHALL return a `CodecError::Encode` error identifying the invalid EPSG code.
11. IF a `"GeoPixelScale"` value is not a JSON array of exactly 3 numbers, THEN THE TIFFDatasetWriter SHALL return a `CodecError::Encode` error describing the expected format.

### Requirement 8: GeoTIFF Read-Write Roundtrip Correctness

**User Story:** As a developer, I want to verify that GeoTIFF metadata written by the writer can be read back with identical values, so that geospatial metadata is preserved through write-read cycles.

#### Acceptance Criteria

1. FOR ALL valid combinations of GeoModelType, GeoRasterType, and EPSG codes, writing GeoTIFF metadata via TIFFDatasetWriter and reading it back via TIFFDatasetReader SHALL produce identical metadata field values (roundtrip property).
2. FOR ALL valid ModelPixelScaleTag values (3-element double arrays with positive scale_x and scale_y), writing and reading back SHALL produce identical `"GeoPixelScale"` values.
3. FOR ALL valid ModelTiepointTag values (arrays of 6-element tuples), writing and reading back SHALL produce identical `"GeoTiepoints"` values.
4. FOR ALL valid ModelTransformationTag values (16-element double arrays), writing and reading back SHALL produce identical `"GeoTransformation"` values.
5. FOR ALL valid GeoTIFF metadata combinations, writing then reading then writing again SHALL produce an equivalent GeoTIFF file (idempotent encoding).

### Requirement 9: GeoTIFF Metadata Value Representation

**User Story:** As a developer, I want GeoTIFF metadata values to use consistent, typed `serde_json::Value` representations, so that metadata values are deterministic and directly usable without string parsing.

#### Acceptance Criteria

1. THE GeoTIFF_Parser SHALL represent `"GeoPixelScale"` as a `serde_json::Value::Array` of 3 `Number` values (e.g., `[0.5, 0.5, 0.0]`).
2. THE GeoTIFF_Parser SHALL represent `"GeoTiepoints"` as a `serde_json::Value::Array` of arrays, each containing 6 `Number` values (e.g., `[[0.0, 0.0, 0.0, 300000.0, 4500000.0, 0.0]]`).
3. THE GeoTIFF_Parser SHALL represent `"GeoTransformation"` as a `serde_json::Value::Array` of 16 `Number` values.
4. THE GeoTIFF_Parser SHALL represent `"GeoModelType"` and `"GeoRasterType"` as `serde_json::Value::String` values.
5. THE GeoTIFF_Parser SHALL represent `"GeoProjectedCRS"` and `"GeoGeographicCRS"` as `serde_json::Value::Number` values.
6. FOR ALL valid GeoTIFF metadata, reading the metadata, writing it back via the writer, and reading again SHALL produce identical `serde_json::Value` representations (value roundtrip).

### Requirement 10: Unit Tests with Synthetic GeoTIFF Data

**User Story:** As a developer, I want unit tests that validate GeoTIFF parsing and writing using synthetic test data, so that correctness is verified without depending on external GeoTIFF files.

#### Acceptance Criteria

1. THE unit tests SHALL include a test that constructs a minimal GeoTIFF byte buffer with GeoKey directory, ModelPixelScaleTag, and ModelTiepointTag, reads it via TIFFDatasetReader, and verifies the expected GeoTIFF metadata fields are present.
2. THE unit tests SHALL include a test that verifies GeoKey values with TIFFTagLocation=0 (inline SHORT), TIFFTagLocation=34736 (double params), and TIFFTagLocation=34737 (ASCII params) are all correctly resolved.
3. THE unit tests SHALL include a test that verifies ModelTransformationTag is parsed into the `"GeoTransformation"` metadata field with the correct 16-element JSON array.
4. THE unit tests SHALL include a test that verifies a plain TIFF (no tag 34735) produces no GeoTIFF metadata fields.
5. THE unit tests SHALL include a test that writes GeoTIFF metadata via TIFFDatasetWriter and reads it back, verifying all fields match.

### Requirement 11: Integration Tests with Real-World GeoTIFF Files

**User Story:** As a developer, I want integration tests that validate GeoTIFF metadata parsing against real-world GeoTIFF files, so that the parser handles production data correctly.

#### Acceptance Criteria

1. THE integration tests SHALL read GeoTIFF files from `data/integration/` and verify that expected metadata fields (GeoModelType, GeoProjectedCRS or GeoGeographicCRS, GeoPixelScale, GeoTiepoints) are present and contain plausible values.
2. THE integration tests SHALL be marked with `pytest.mark.integration` so they can be run separately from unit tests.
3. THE integration tests SHALL skip gracefully when integration test data is not available (the `data/integration/` directory is gitignored).
