# VirtualiZarr Parsers

VirtualiZarr parser for generating virtual Zarr datasets from imagery files.

`OversightMLParser` implements the VirtualiZarr `Parser` protocol and produces
`ManifestStore` objects that can be serialized to Kerchunk JSON indices. It works
for any format supported by `IO.open()`: NITF, standalone JPEG 2000, TIFF, and
GeoTIFF.

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


