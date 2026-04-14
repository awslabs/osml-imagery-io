# VirtualiZarr Parsers

VirtualiZarr parser for generating virtual Zarr datasets from imagery files.

`OversightMLParser` implements the VirtualiZarr `Parser` protocol and produces
`ManifestStore` objects that can be serialized to Kerchunk JSON indices. It works
for any format supported by `IO.open()`: NITF, standalone JPEG 2000, TIFF, and
GeoTIFF.

The parser supports both single-file and multi-file inputs:

- **Single file** — pass a single path and URL. If the file contains overview
  assets (e.g. COG overview IFDs), the parser builds a hierarchical store
  automatically. Otherwise it produces a flat store.
- **Multi-file pyramid** — pass a list of paths and URLs, one per resolution
  level. The parser builds a hierarchical store with GeoZarr `multiscales`
  metadata describing the pyramid structure.

```{note}
`virtualizarr` is an optional dependency. Install with `pip install osml-imagery-io[virtualizarr]`
to enable parser support.
```

## OversightMLParser

```{eval-rst}
.. autoclass:: aws.osml.io.virtualizarr_parsers.OversightMLParser
   :members:
   :undoc-members:
   :show-inheritance:
```

### Constructor

`OversightMLParser(local_paths)` accepts either a single path string or a list
of paths. A single string is wrapped in a list internally.

```python
# Single file
parser = OversightMLParser(local_paths="/data/image.ntf")

# Multi-file pyramid (one file per resolution level)
parser = OversightMLParser(local_paths=["/data/image.ntf", "/data/image.ntf.r1"])
```

### Calling the parser

`parser(url)` accepts either a single URL string or a list of URLs. A single
URL is used for all chunk references. A list must have the same length as
`local_paths` — each URL corresponds to the local path at the same index.

```python
# Single URL — used for all assets
store = parser(url="s3://bucket/image.ntf")

# Multiple URLs — one per file in the pyramid
store = parser(url=["s3://bucket/image.ntf", "s3://bucket/image.ntf.r1"])
```

### Flat vs hierarchical output

When the parser detects overview assets (keys matching `image:N:overview:M`),
it produces a **hierarchical** `ManifestStore` with one subgroup per resolution
level. Otherwise it produces a **flat** store with arrays at the root — identical
to the pre-multiscale behavior.

For hierarchical stores, each subgroup contains a single array named `"data"`,
and the root group's attributes include GeoZarr `multiscales` metadata and a
`zarr_conventions` array declaring convention identity:

```
ManifestGroup (root)
├── groups:
│   ├── "0" → ManifestGroup(arrays={"data": level_0_array})
│   ├── "1" → ManifestGroup(arrays={"data": level_1_array})
│   └── "2" → ManifestGroup(arrays={"data": level_2_array})
└── attributes:
    ├── "source": "s3://bucket/image.ntf"
    ├── "zarr_conventions": [{ ... }]
    └── "multiscales": { ... }
```

### `multiscales` metadata structure

The root group's `multiscales` attribute conforms to the
[GeoZarr multiscales convention](https://github.com/zarr-conventions/multiscales)
(UUID `d35379db-88df-4056-af3a-620245f8e347`). It contains:

- **layout** — one entry per resolution level with an `asset` path matching the
  subgroup name, an optional `derived_from` referencing the parent level, and a
  `transform` object with relative `scale` and `translation` arrays
- **resampling_method** — optional; recorded when a `downsampling_method` keyword
  argument is provided to the parser

Scale transforms use relative factors between adjacent levels (not absolute from
level 0). The `scale` and `translation` arrays have two elements: `[Y, X]`.

A `zarr_conventions` array in the root attributes declares convention identity:

```json
{
  "source": "s3://bucket/image.tif",
  "zarr_conventions": [
    {
      "uuid": "d35379db-88df-4056-af3a-620245f8e347",
      "schema_url": "https://raw.githubusercontent.com/zarr-conventions/multiscales/refs/tags/v1/schema.json",
      "spec_url": "https://github.com/zarr-conventions/multiscales/blob/v1/README.md",
      "name": "multiscales",
      "description": "Multiscale layout of zarr datasets"
    }
  ],
  "multiscales": {
    "layout": [
      {
        "asset": "0",
        "transform": {"scale": [1.0, 1.0], "translation": [0.0, 0.0]}
      },
      {
        "asset": "1",
        "derived_from": "0",
        "transform": {"scale": [2.0, 2.0], "translation": [0.0, 0.0]}
      }
    ],
    "resampling_method": "average"
  }
}
```

## write_tile_index

```{eval-rst}
.. autofunction:: aws.osml.io.virtualizarr_parsers.write_tile_index
```

`write_tile_index()` automatically detects whether the store is flat or
hierarchical and serializes accordingly. For hierarchical stores, the output
Kerchunk JSON uses path-prefixed keys (e.g. `0/data/0.0.0`, `1/data/0.0.0`)
and includes the root `multiscales` metadata in `.zattrs`.


