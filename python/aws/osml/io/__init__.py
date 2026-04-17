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
    StructureRegistry,
    StructureWriter,
    TextAssetProvider,
    Value,
    __version__,
    decode_tiff_tile,
)
from aws.osml.io.convenience import ImageInfo, Tile, iminfo, imread, imsave, tiles

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
    # Codec decode functions
    "decode_tiff_tile",
    # Convenience API
    "imread",
    "imsave",
    "iminfo",
    "tiles",
    "ImageInfo",
    "Tile",
]

# Optional VirtualiZarr parser exports — only available when virtualizarr is installed
try:
    from aws.osml.io.virtualizarr_parsers import OversightMLParser, write_tile_index

    __all__ += ["OversightMLParser", "write_tile_index"]
except ImportError:
    pass
