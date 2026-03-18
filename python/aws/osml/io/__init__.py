"""AWS OSML IO - Geospatial image format codecs.

This package provides high-performance image format decoders and encoders
for geospatial imagery formats including NITF and GeoTIFF.
"""

from aws.osml.io._io import (
    IO,
    AssetProvider,
    AssetType,
    BufferedImageAssetProvider,
    BufferedMetadataProvider,
    BufferedTextAssetProvider,
    DataAssetProvider,
    DatasetReader,
    DatasetWriter,
    GraphicsAssetProvider,
    ImageAssetProvider,
    MetadataProvider,
    PixelType,
    StructureAccessor,
    StructureDefinition,
    # Parser bindings
    StructureRegistry,
    StructureWriter,
    TextAssetProvider,
    Value,
    __version__,
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
    "BufferedImageAssetProvider",
    "TextAssetProvider",
    "BufferedTextAssetProvider",
    "DataAssetProvider",
    "GraphicsAssetProvider",
    # Metadata
    "MetadataProvider",
    "BufferedMetadataProvider",
    # Parser
    "StructureRegistry",
    "StructureAccessor",
    "StructureWriter",
    "StructureDefinition",
    "Value",
]
