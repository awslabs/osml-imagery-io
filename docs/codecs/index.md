# Codec Specifications

Zarr v3 codec specifications for osml-imagery-io. Each codec is identified by a URI-based
name per the Zarr v3 (ZEP9) specification. These pages describe the codec configuration
schema and decoding behavior.

| Codec | URI |
|-------|-----|
| JPEG 2000 | `https://awslabs.github.io/osml-imagery-io/codecs/jpeg2000` |
| JPEG | `https://awslabs.github.io/osml-imagery-io/codecs/jpeg` |
| JBP Block | `https://awslabs.github.io/osml-imagery-io/codecs/jbp-block` |
| TIFF Tile | `https://awslabs.github.io/osml-imagery-io/codecs/tiff-tile` |
| DTED | `https://awslabs.github.io/osml-imagery-io/codecs/dted` |

```{toctree}
:maxdepth: 1

jpeg2000
jpeg
jbp-block
tiff-tile
dted
```
