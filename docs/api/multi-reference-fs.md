# MultiReferenceFileSystem

An fsspec filesystem that extends Kerchunk's ``ReferenceFileSystem`` with
multi-range byte fetching. This is used internally by the Zarr codec pipeline
to read JPEG 2000 codestreams with interleaved tile-parts that span
non-contiguous byte ranges in a single file.

```{note}
`fsspec` is an optional dependency. This class is available when
`pip install osml-imagery-io[zarr]` is installed.
```

```{eval-rst}
.. autoclass:: aws.osml.io.multi_reference_fs.MultiReferenceFileSystem
   :members:
   :undoc-members:
   :show-inheritance:
```
