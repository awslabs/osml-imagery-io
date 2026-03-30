# Zarr Codecs

Zarr v3 codec plugins for decoding JPEG 2000, JPEG, and uncompressed JBP/NITF imagery.

These codecs implement the zarr-python v3 codec protocol and are registered via Python entry points
for automatic discovery by the Zarr codec registry. They enable reading cloud-hosted NITF and TIFF
imagery through `xarray.open_zarr()` using Kerchunk indices.

```{note}
`zarr` is an optional dependency. Install with `pip install osml-imagery-io[zarr]` to enable
Zarr codec support.
```

## Codec Classes

### Jpeg2000Codec

```{eval-rst}
.. autoclass:: aws.osml.io.zarr_codecs.Jpeg2000Codec
   :members:
   :undoc-members:
   :show-inheritance:
```

### JpegCodec

```{eval-rst}
.. autoclass:: aws.osml.io.zarr_codecs.JpegCodec
   :members:
   :undoc-members:
   :show-inheritance:
```

### JbpBlockCodec

```{eval-rst}
.. autoclass:: aws.osml.io.zarr_codecs.JbpBlockCodec
   :members:
   :undoc-members:
   :show-inheritance:
```

## Decode Binding Functions

### decode_jpeg2000

```{eval-rst}
.. autofunction:: aws.osml.io.zarr_codecs.decode_jpeg2000
```

### decode_jpeg

```{eval-rst}
.. autofunction:: aws.osml.io.zarr_codecs.decode_jpeg
```

### decode_jbp_block

```{eval-rst}
.. autofunction:: aws.osml.io.zarr_codecs.decode_jbp_block
```
