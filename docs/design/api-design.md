# OversightML Imagery IO API Design: Tiled Image Pyramid Access

This document presents the API design for OversightML's low-level access to large tiled image pyramids. The API combines concepts from the National Imagery Transmission Format (NITF) specification with ideas from SpatioTemporal Asset Catalogs (STAC) to provide a framework for geospatial imagery access.

For usage examples and practical guidance, see the [User Guide](../user-guide/index.md).

## Overview

## Core API Structure

The API models **Datasets** as collections of related assets (images, graphics, text, data), each with its own metadata. Assets are accessed by string keys rather than numeric indices, enabling discovery and categorization while remaining flexible enough to represent format-specific data models like the Joint BIIF Profile (JBP).

The `DatasetReader` and `DatasetWriter` abstract classes provide the main entry points, while the `IO` class serves as a factory that selects the appropriate implementation based on file format detection.

```mermaid
classDiagram
direction TB
    class DatasetReader {
        <<abstract>>
	    +get_asset(key: str) AssetProvider
	    +get_asset_keys(asset_type: Optional[AssetType], roles: Optional[List[str]]) List[str]
	    +has_asset(key: str) bool
	    +metadata MetadataProvider
	    +close() None
	    +__enter__() DatasetReader
	    +__exit__() None
    }
    class DatasetWriter {
        <<abstract>>
	    +add_asset(key: str, provider: AssetProvider, title: str, description: str, roles: List[str]) None
	    +metadata MetadataProvider
	    +close() None
	    +__enter__() DatasetWriter
	    +__exit__() None
    }


    class IO {
        +open(paths: List[str], mode: str, format: Optional[str]) Union[DatasetReader, DatasetWriter]
    }

    DatasetReader --> AssetProvider : provides
    DatasetReader --> MetadataProvider : provides
    DatasetWriter --> AssetProvider : consumes
    DatasetWriter --> MetadataProvider : consumes
    AssetProvider --> MetadataProvider : provides
    IO --> DatasetReader : provides
    IO --> DatasetWriter : provides

  	<<abstract>> AssetProvider
	<<abstract>> MetadataProvider
```

## Asset Provider Hierarchy

The Asset Provider hierarchy handles different content types found in geospatial datasets. The base `AssetProvider` class establishes common metadata and organizational elements that all assets share, including keys, titles, descriptions, media types, and roles for discovery and categorization. Specialized providers extend this with type-specific functionality: `ImageAssetProvider` offers blocked access for processing large imagery, `TextAssetProvider` handles encoding and format-specific text retrieval, `DataAssetProvider` provides parsing for structured data like XML and JSON, and `GraphicsAssetProvider` manages vector graphics and annotations. This hierarchy allows datasets to function as self-describing collections. 

```mermaid
classDiagram
direction TB
    class AssetProvider {
        <<abstract>>
        +key str
        +title str
        +description str
        +media_type str
        +roles List[str]
        +asset_type AssetType
        +raw_asset BytesIO
        +metadata MetadataProvider
        +from_bytes(data: bytes, key: str, media_type: str)$ AssetProvider
    }

    class MetadataProvider {
        +raw BytesIO
	    +entries(prefix: Optional[str]) Dict[str, Any]
	    +get(key: str, default=None) Any
	    +keys() list[str]
	    +values() list[Any]
	    +items() list[tuple[str, Any]]
    }

    class ImageAssetProvider {
        <<abstract>>
        +has_block(block_row: int, block_col: int, resolution_level: int) bool
        +get_block(block_row: int, block_col: int, resolution_level: int, bands: Optional[List[int]]) ndarray
        +num_resolution_levels int
        +num_bands int
        +num_rows int
        +num_columns int
        +num_pixels_per_block_horizontal int
        +num_pixels_per_block_vertical int
        +num_bits_per_pixel int
        +actual_bits_per_pixel int
        +pixel_value_type dtype
        +pad_pixel_value Number
        +image_shape Tuple[int, int, int]
        +block_shape Tuple[int, int, int]
        +block_grid_size Tuple[int, int]
    }

    class TextAssetProvider {
        <<abstract>>
        +text str
        +encoding str
        +format str
    }

    class DataAssetProvider {
        <<abstract>>
        +mime_type str
        +parse_as_xml() ElementTree
        +parse_as_json() Dict[str, Any]
    }

    class GraphicsAssetProvider {
        <<abstract>>
        +raw_asset BytesIO
    }

    AssetProvider <|-- ImageAssetProvider
    AssetProvider <|-- TextAssetProvider
    AssetProvider <|-- GraphicsAssetProvider
    AssetProvider <|-- DataAssetProvider
    AssetProvider --> MetadataProvider : provides

```

## ImageAssetProvider Hierarchy

The ImageAssetProvider hierarchy supports multiple image compression formats and data sources through a common blocked access interface. Each concrete implementation handles the decoding and access patterns required for its format while presenting a consistent API for blocked image data retrieval. The `BufferedImageAssetProvider` enables in-memory processing workflows, while format-specific providers like `JBPImageAssetProvider`, `J2KImageAssetProvider`, and `TIFFImageAssetProvider` provide lazy decoding and encoding for specific compression schemes and file structures. This design allows applications to work with different image formats—JPEG 2000 compressed imagery in NITF files, standard TIFF pyramids, or data generated in memory—through the same interface.

```mermaid
classDiagram
direction LR
    class ImageAssetProvider {
        <<abstract>>
        +has_block(block_row: int, block_col: int, resolution_level: int) bool
        +get_block(block_row: int, block_col: int, resolution_level: int, bands: Optional[List[int]]) ndarray
        +num_resolution_levels int
        +num_bands int
        +num_rows int
        +num_columns int
        +num_pixels_per_block_horizontal int
        +num_pixels_per_block_vertical int
        +num_bits_per_pixel int
        +actual_bits_per_pixel int
        +pixel_value_type dtype
        +pad_pixel_value Number
        +image_shape Tuple[int, int, int]
        +block_shape Tuple[int, int, int]
        +block_grid_size Tuple[int, int]
    }

    class BufferedImageAssetProvider {
        +create(key: str, num_columns: int, num_rows: int, num_bands: int, block_width: int, block_height: int, pixel_type: PixelType, actual_bits_per_pixel: Optional[int], metadata: Optional[MetadataProvider], title: Optional[str], description: Optional[str])$ BufferedImageAssetProvider
        +set_full_image(data: ndarray) None
        +set_full_image_u16(data: ndarray) None
        +set_block(block_row: int, block_col: int, data: bytes) None
    }

    class JBPImageAssetProvider {
        +__init__(key: str, file_handle: BinaryIO, ifd_offset: int, title: str, roles: List[str])
    }

    class J2KImageAssetProvider {
        +__init__(key: str, file_handle: BinaryIO, ifd_offset: int, title: str, roles: List[str])
    }

    class JPEGImageAssetProvider {
        +__init__(key: str, file_handle: BinaryIO, ifd_offset: int, title: str, roles: List[str])
    }

    class TIFFImageAssetProvider {
        +__init__(key: str, file_handle: BinaryIO, ifd_offset: int, title: str, roles: List[str])
    }

    class PNGImageAssetProvider {
        +__init__(key: str, file_handle: BinaryIO, ifd_offset: int, title: str, roles: List[str])
    }


    ImageAssetProvider <|-- JBPImageAssetProvider
    ImageAssetProvider <|-- J2KImageAssetProvider
    ImageAssetProvider <|-- JPEGImageAssetProvider
    ImageAssetProvider <|-- TIFFImageAssetProvider
    ImageAssetProvider <|-- PNGImageAssetProvider
    ImageAssetProvider <|-- BufferedImageAssetProvider
```

For block access patterns, resolution levels, and pixel data format details, see the [Image Assets](../user-guide/image-assets.md) and [Working with Pixels](../user-guide/working-with-pixels.md) user guides.

## GraphicsAssetProvider

The `GraphicsAssetProvider` interface provides access to vector graphics data within geospatial datasets. In NITF files, graphic segments contain CGM (Computer Graphics Metafile) data representing annotations, overlays, and vector graphics that can be rendered on top of imagery.

### Interface Design

The `GraphicsAssetProvider` trait extends `AssetProvider` without adding additional methods. This minimal design reflects that:

1. Raw CGM data is accessed through the inherited `raw_asset()` method
2. Graphic-specific metadata (display level, attachment level, location, bounds) is accessed via the `metadata()` Mapping interface
3. The library extracts raw CGM bytes but does not parse CGM content—users provide their own CGM parsing libraries

```mermaid
classDiagram
direction TB
    class AssetProvider {
        <<abstract>>
        +key str
        +title str
        +description str
        +media_type str
        +roles List[str]
        +asset_type AssetType
        +raw_asset BytesIO
        +metadata MetadataProvider
    }

    class GraphicsAssetProvider {
        <<abstract>>
    }

    class JBPGraphicsAssetProvider {
        -key: String
        -title: String
        -description: String
        -roles: Vec~String~
        -location: SegmentLocation
        -data: Arc~[u8]~
        -metadata: Arc~MetadataProvider~
    }

    AssetProvider <|-- GraphicsAssetProvider
    GraphicsAssetProvider <|.. JBPGraphicsAssetProvider
```

For usage examples, see the [Graphics Assets](../user-guide/graphics-assets.md) user guide.

## TextAssetProvider

The `TextAssetProvider` interface provides access to text content within geospatial datasets. In NITF files, text segments contain textual data with associated metadata for character encoding and display properties. The interface handles encoding-aware text retrieval and line delimiter normalization.

### Interface Design

The `TextAssetProvider` trait extends `AssetProvider` with text-specific methods for accessing decoded content and encoding information:

```mermaid
classDiagram
direction TB
    class AssetProvider {
        <<abstract>>
        +key str
        +title str
        +description str
        +media_type str
        +roles List[str]
        +asset_type AssetType
        +raw_asset BytesIO
        +metadata MetadataProvider
    }

    class TextAssetProvider {
        <<abstract>>
        +text str
        +encoding str
        +format str
    }

    class JBPTextAssetProvider {
        -key: String
        -title: String
        -description: String
        -roles: Vec~String~
        -location: SegmentLocation
        -data: Arc~[u8]~
        -metadata: Arc~MetadataProvider~
        -txtfmt: String
    }

    class BufferedTextAssetProvider {
        -key: String
        -title: String
        -description: String
        -roles: Vec~String~
        -text_content: String
        -encoding: String
        -metadata: Arc~MetadataProvider~
    }

    AssetProvider <|-- TextAssetProvider
    TextAssetProvider <|.. JBPTextAssetProvider
    TextAssetProvider <|.. BufferedTextAssetProvider
```

For usage examples, see the [Text Assets](../user-guide/text-assets.md) user guide.

## Writer API: Why Encoding Hints Use Metadata

The writer side of the API uses `BufferedMetadataProvider` to control how images are encoded when written to disk. This design keeps format-specific parameters out of abstract interfaces:

```mermaid
flowchart LR
    A[BufferedMetadataProvider] -->|"set('IC', 'C8')"| B[metadata storage]
    B --> C[BufferedImageAssetProvider]
    C -->|"metadata()"| D[DatasetWriter]
    D -->|"reads IC, IMODE, etc."| E[Encoder Selection]
    E --> F[Output File]
```

1. **Clean abstractions**: `BufferedImageAssetProvider` doesn't need NITF-specific parameters
2. **Seamless copying**: Metadata from a reader can flow directly to a writer
3. **Consistent naming**: The same field names used when reading are used when writing
4. **Format flexibility**: Different output formats read different hint fields

The writer knows what format it's writing, so it knows which metadata fields to look for. This allows the same `BufferedImageAssetProvider` to be written to NITF, GeoTIFF, or other formats by simply changing the writer and the encoding hints.

For encoding options, chipping/transcoding workflows, and masked image support, see the [Writing Imagery Assets](../user-guide/image-assets-writing.md) user guide.

## ImageOperation Pattern for Large Image Processing

The ImageOperation pattern applies image processing algorithms to large geospatial imagery without loading entire images into memory. This design implements the ImageAssetProvider interface, allowing operations to be chained and composed while maintaining the same blocked access patterns as the underlying data sources. The `ImageOperation` class wraps any callable function (such as scikit-image filters) and applies it block-by-block as data is requested, enabling integration with existing image processing libraries. The pattern supports both simple per-block operations and neighborhood-based algorithms through its caching and block retrieval mechanisms, allowing processing pipelines that scale to large imagery datasets.

```mermaid
classDiagram
direction TD
    class ImageOperation {
        -input_provider: ImageAssetProvider
        -operation_func: Callable
        -operation_kwargs: Dict
        -cache: Dict[Tuple[int, int], ndarray]
        
        +__init__(key: str, input_provider: ImageAssetProvider, operation_func: Callable, **kwargs)
        +has_block(block_row: int, block_col: int, resolution_level: int) bool
        +get_block(block_row: int, block_col: int, resolution_level: int, bands: List[int]) ndarray
        +from_function(func: Callable, **kwargs) ImageOperation
        +chain(other_operation: ImageOperation) ImageOperation
        -apply_operation_to_block(block: ndarray) ndarray
        -get_neighborhood_blocks(block_row: int, block_col: int, radius: int) List[ndarray]
    }

    ImageAssetProvider <|-- ImageOperation
    ImageOperation --> ImageAssetProvider : consumes
```

## Format-Specific Implementations

The abstract DatasetReader/DatasetWriter and AssetProvider interfaces enable support for different geospatial formats through concrete implementations. Each format provides its own reader/writer classes and asset providers that handle format-specific encoding details.

The Joint BIIF Profile (JBP) format, which includes NITF and NSIF files, demonstrates how the abstract interfaces work with a multi-asset format that supports various compression schemes. In these formats multiple assets are represented as segments of a single combined file.

```mermaid
classDiagram
direction TB
    class JBPDatasetReader {
        -input_path: Path
        
        +__init__(paths: List[Path])
    }

    class JBPDatasetWriter {
        -output_path: Path
        
        +__init__(path: Path)
    }

    DatasetReader <|-- JBPDatasetReader
    DatasetWriter <|-- JBPDatasetWriter
  	<<abstract>> DatasetReader
	<<abstract>> DatasetWriter

```

## Parser Infrastructure (PyStructure Classes)

The parser infrastructure provides a data-driven approach to reading and writing binary structures. Instead of hand-coding parsers for each format, structure definitions are loaded from KSY (Kaitai Struct YAML) files and used to parse binary data at runtime. This enables maintainable parsing of formats like NITF headers and TRE extensions.

```mermaid
classDiagram
direction TB
    class StructureRegistry {
        +__init__()
        +add_search_path(path: str) None
        +get(name: str) Optional[StructureDefinition]
        +list() List[str]
        +reload() None
        +register(name: str, definition: StructureDefinition) None
        +search_paths() List[str]
    }

    class StructureDefinition {
        +id str
        +title Optional[str]
        +field_names List[str]
    }

    class StructureAccessor {
        +__init__(definition: StructureDefinition, data: bytes)
        +__getitem__(path: str) Value
        +has(path: str) bool
        +fields() List[str]
        +raw_view(path: str) bytes
        +field_byte_range(path: str) Tuple[int, int]
        +data bytes
        +definition StructureDefinition
    }

    class StructureWriter {
        +new_fixed(definition: StructureDefinition)$ StructureWriter
        +new_streaming(definition: StructureDefinition)$ StructureWriter
        +__setitem__(path: str, value: Any) None
        +set(path: str, value: Any) None
        +is_set(path: str) bool
        +finish() bytes
        +buffer() bytes
    }

    class Value {
        +as_str() str
        +as_int() int
        +as_float() float
        +as_bytes() bytes
    }

    StructureRegistry --> StructureDefinition : provides
    StructureAccessor --> StructureDefinition : uses
    StructureAccessor --> Value : returns
    StructureWriter --> StructureDefinition : uses
```

For parser usage examples and structure definition authoring, see the [Metadata](../user-guide/metadata.md) user guide.
