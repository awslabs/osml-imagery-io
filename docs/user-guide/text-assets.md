# Text Assets

Text assets store plain text content within a dataset — mission reports, annotations,
processing notes, and similar human-readable data.

## Reading Text Assets

```python
from aws.osml.io import IO

with IO.open(["image.ntf"], "r") as dataset:
    for key in dataset.get_asset_keys(asset_type="text"):
        text = dataset.get_asset(key)
        print(f"Text '{key}': {text.text[:200]}...")
```

## Writing Text Assets

```python
from aws.osml.io import IO, BufferedTextAssetProvider

text_asset = BufferedTextAssetProvider(
    key="text_segment_0",
    text_content="Mission report content...",
    encoding="UTF-8",
).with_title("Mission Report")

with IO.open(["output.ntf"], "w", "nitf") as writer:
    writer.add_asset("text_segment_0", text_asset,
                     title="Mission Report",
                     description="Operational text",
                     roles=["data", "annotation"])
```
