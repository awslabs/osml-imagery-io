# Data Assets

Data assets carry structured payloads alongside imagery. Common uses include XML
metadata (SICD/SIDD), overflow TREs, and application-specific data.

## Reading Data Assets

```python
from aws.osml.io import IO, AssetType

with IO.open(["image.ntf"], "r") as dataset:
    for key in dataset.get_asset_keys(asset_type=AssetType.Data):
        data = dataset.get_asset(key)
        print(f"Data '{key}': mime_type={data.mime_type}")
```

## SICD/SIDD XML Example

SAR imagery standards store complex XML metadata in data assets. The library can parse
these directly:

```python
from aws.osml.io import IO, AssetType

with IO.open(["sicd_image.ntf"], "r") as dataset:
    for key in dataset.get_asset_keys(asset_type=AssetType.Data):
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

Use `BufferedDataAssetProvider` to create data assets with full control over DES
subheader fields. Attach a `BufferedMetadataProvider` to set DESID, DESVER, and
security fields.

### SICD/SIDD XML Metadata

```python
from aws.osml.io import BufferedDataAssetProvider, BufferedMetadataProvider, IO

# Prepare DES metadata (required for valid SICD/SIDD)
meta = BufferedMetadataProvider()
meta.set("DESID", "XML_DATA_CONTENT")
meta.set("DESVER", "01")

# Load your SICD XML (from file, generation, etc.)
with open("sicd_metadata.xml", "rb") as f:
    sicd_xml_bytes = f.read()

# Create the data asset
data_asset = BufferedDataAssetProvider.create(
    key="des:0",
    data=sicd_xml_bytes,
    mime_type="application/xml",
    title="SICD Metadata",
    roles=["metadata"],
    metadata=meta,
)

# Write to a NITF file
with IO.open(["output.ntf"], "w") as writer:
    writer.add_asset("des:0", data_asset, "SICD Metadata", "", ["metadata"])
```

### Binary or JSON Payloads

```python
import json
from aws.osml.io import BufferedDataAssetProvider, BufferedMetadataProvider, IO

meta = BufferedMetadataProvider()
meta.set("DESID", "APP_CONFIG")
meta.set("DESVER", "01")

config = {"processing_level": 3, "sensor_id": "SAR-X1"}
payload = json.dumps(config).encode("utf-8")

data_asset = BufferedDataAssetProvider.create(
    key="des:0",
    data=payload,
    mime_type="application/json",
    metadata=meta,
)

with IO.open(["output.ntf"], "w") as writer:
    writer.add_asset("des:0", data_asset, "Config", "", ["metadata"])
```

### Field Validation

DESID must be 1–25 characters and DESVER must be exactly 2 characters. Invalid
values raise an error at write time:

```python
meta = BufferedMetadataProvider()
meta.set("DESID", "A" * 26)  # Too long — will raise at write time
meta.set("DESVER", "1")      # Must be exactly 2 chars — will raise
```
