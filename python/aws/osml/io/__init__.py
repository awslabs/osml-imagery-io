"""AWS OSML IO - Geospatial image format codecs.

This package provides high-performance image format decoders and encoders
for geospatial imagery formats including NITF and GeoTIFF.
"""

from aws.osml.io._io import (
    __version__,
    AssetType,
    PixelType,
    IO,
    DatasetReader,
    DatasetWriter,
    AssetProvider,
    ImageAssetProvider,
    MemoryImageAssetProvider,
    TextAssetProvider,
    DataAssetProvider,
    GraphicsAssetProvider,
    MetadataProvider,
    SimpleMetadataProvider,
    # Parser bindings
    StructureRegistry,
    StructureAccessor,
    StructureWriter,
    StructureDefinition,
    Value,
)

# Convenience alias for IO.open
open = IO.open

__all__ = [
    "__version__",
    # Enumerations
    "AssetType",
    "PixelType",
    # IO Factory
    "IO",
    "open",
    # Reader/Writer
    "DatasetReader",
    "DatasetWriter",
    # Asset Providers
    "AssetProvider",
    "ImageAssetProvider",
    "MemoryImageAssetProvider",
    "TextAssetProvider",
    "DataAssetProvider",
    "GraphicsAssetProvider",
    # Metadata
    "MetadataProvider",
    "SimpleMetadataProvider",
    # Parser
    "StructureRegistry",
    "StructureAccessor",
    "StructureWriter",
    "StructureDefinition",
    "Value",
]
