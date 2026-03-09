# Data Assets

Data assets carry structured payloads alongside imagery. Common uses include XML
metadata (SICD/SIDD), overflow TREs, and application-specific data.

## Reading Data Assets

```python
from aws.osml.io import IO

with IO.open(["image.ntf"], "r") as dataset:
    for key in dataset.get_asset_keys(asset_type="data"):
        data = dataset.get_asset(key)
        print(f"Data '{key}': mime_type={data.mime_type}")
```

## SICD/SIDD XML Example

SAR imagery standards store complex XML metadata in data assets. The library can parse
these directly:

```python
from aws.osml.io import IO

with IO.open(["sicd_image.ntf"], "r") as dataset:
    for key in dataset.get_asset_keys(asset_type="data"):
        data = dataset.get_asset(key)
        if data.mime_type == "application/xml":
            xml_tree = data.parse_as_xml()
            root = xml_tree.getroot()
            print(f"XML root tag: {root.tag}")

            # Navigate the XML tree
            for child in root:
                print(f"  {child.tag}")
```

## Reading Raw Data

For non-XML payloads, access the raw bytes:

```python
with IO.open(["image.ntf"], "r") as dataset:
    data = dataset.get_asset("data_segment_0")
    raw_bytes = data.raw_asset.read()
```

## Writing Data Assets

<!-- TODO: BufferedDataAssetProvider example for writing data assets -->
