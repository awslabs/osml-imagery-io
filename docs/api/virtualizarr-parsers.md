# VirtualiZarr Parsers

VirtualiZarr parser for generating virtual Zarr datasets from imagery files.

`OversightMLParser` implements the VirtualiZarr `Parser` protocol and produces
`ManifestStore` objects that can be serialized to Kerchunk JSON indices. It works
for any format supported by `IO.open()`: NITF, standalone JPEG 2000, TIFF, and
GeoTIFF.

The parser conforms to the VirtualiZarr parser callable protocol
`(url, registry) -> ManifestStore`. It reads bytes by opening `url` with fsspec
and handing the seekable handle to `IO.open()`, which issues on-demand byte-range
reads for the block-capable formats — so a local path and an `s3://` URL follow
the same code path and neither downloads the whole file to build the index.

- **Single file** — pass one URL. If the file contains overview assets (e.g. COG
  overview IFDs), the parser builds a hierarchical store automatically.
- **Multi-file pyramid** — pass the base URL; sibling `.r1`/`.r2`/… R-set
  companions are discovered on the same filesystem and mapped to overview levels.
  The parser builds a hierarchical store with GeoZarr `multiscales` metadata
  describing the pyramid structure.

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

`OversightMLParser()` takes no parse-time configuration — the URL passed when
the parser is called is the single source of truth for both reading and chunk
references.

```python
parser = OversightMLParser()
```

### Calling the parser

`parser(url, registry=None)` reads and indexes the imagery at `url`. Local
paths, `file://` URIs, and `s3://` URIs all work (opened via fsspec). Chunk
references in the returned store point at `url`. R-set overview companions
(`<url>.r1`, `<url>.r2`, …) are discovered automatically.

```python
# Local file (chunk refs point at the local path)
store = parser("/data/image.ntf")

# Remote file — range reads, no full download
store = parser("s3://bucket/image.ntf")

# Multi-file pyramid — image.ntf.r1 etc. auto-discovered from the base URL
store = parser("s3://bucket/image.ntf")
```

To relocate chunk references (portable `{{base}}` indexes, or index a local copy
but reference an `s3://` location), use `write_tile_index`'s `template_base` /
`url_overrides` arguments — see below.

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

Relocating chunk references is a serialization-time concern controlled by two
mutually exclusive keyword arguments:

- **`template_base`** — pass `"{{base}}"` to produce a portable index whose
  chunk-reference URLs are rewritten to `{{base}}<filename>` and emit a Kerchunk
  v1 `"templates": {"base": ""}` dict. At read time the base is supplied via
  `template_overrides={"base": "s3://bucket/path/"}`.
- **`url_overrides`** — an explicit `{old_url: new_url}` mapping, e.g. index a
  local copy and point the references at the `s3://` location the data will be
  served from.

```python
# Portable index (resolve base URL at read time)
parser = OversightMLParser()
store = parser("local/image.ntf")
write_tile_index(store, "image.json", template_base="{{base}}")

# Index a local copy, reference the remote location
write_tile_index(
    store, "image.json",
    url_overrides={os.path.abspath("local/image.ntf"): "s3://bucket/image.ntf"},
)
```


