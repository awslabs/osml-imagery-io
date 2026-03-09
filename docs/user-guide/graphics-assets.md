# Graphics Assets

Graphics assets contain vector overlay data (typically CGM format). The library provides
raw, unparsed access to the graphic data bytes.

## Reading Graphics Assets

```python
from aws.osml.io import IO

with IO.open(["image.ntf"], "r") as dataset:
    for key in dataset.get_asset_keys(asset_type="graphics"):
        graphic = dataset.get_asset(key)
        cgm_data = graphic.raw_asset.read()
        print(f"Graphics '{key}': {len(cgm_data)} bytes")
```

Parsing the CGM content is left to the application — the library does not interpret
the graphic data beyond providing access to the raw bytes.
