# Asset Providers

Every asset in a dataset is represented by a typed provider. When you call
{meth}`DatasetReader.get_asset <aws.osml.io.DatasetReader.get_asset>`, the
library returns a specialised provider whose type matches the asset's category:

| Asset category | Returned type |
|----------------|---------------|
| Image | {class}`~aws.osml.io.ImageAssetProvider` |
| Text | {class}`~aws.osml.io.TextAssetProvider` |
| Data | {class}`~aws.osml.io.DataAssetProvider` |
| Graphics | {class}`~aws.osml.io.GraphicsAssetProvider` |

All provider types share a common set of properties — `key`, `title`,
`description`, `media_type`, `roles`, and `asset_type` — while each
specialised type adds format-specific access methods (e.g. block-level image
reads, text content, structured data parsing).

## AssetProvider

`AssetProvider` is the base class exposing the common metadata properties.
Use {meth}`AssetProvider.from_bytes` to create an asset from raw bytes for
writing (for image assets, use {class}`~aws.osml.io.BufferedImageAssetProvider`
instead).

```{eval-rst}
.. autoclass:: aws.osml.io.AssetProvider
   :members:
   :undoc-members:
   :show-inheritance:
```

## ImageAssetProvider

Returned by {meth}`~aws.osml.io.DatasetReader.get_asset` for image assets.
Provides block-level and full-image read access.

```{eval-rst}
.. autoclass:: aws.osml.io.ImageAssetProvider
   :members:
   :undoc-members:
   :show-inheritance:
```

## BufferedImageAssetProvider

An in-memory image asset provider. Use this to create image assets for writing
via {meth}`~aws.osml.io.DatasetWriter.add_asset`.

```{eval-rst}
.. autoclass:: aws.osml.io.BufferedImageAssetProvider
   :members:
   :undoc-members:
   :show-inheritance:
```

## TextAssetProvider

Returned by {meth}`~aws.osml.io.DatasetReader.get_asset` for text assets.
Provides text content and encoding information.

```{eval-rst}
.. autoclass:: aws.osml.io.TextAssetProvider
   :members:
   :undoc-members:
   :show-inheritance:
```

## BufferedTextAssetProvider

An in-memory text asset provider. Use this to create text assets for writing
via {meth}`~aws.osml.io.DatasetWriter.add_asset`.

```{eval-rst}
.. autoclass:: aws.osml.io.BufferedTextAssetProvider
   :members:
   :undoc-members:
   :show-inheritance:
```

## GraphicsAssetProvider

Returned by {meth}`~aws.osml.io.DatasetReader.get_asset` for graphics assets.

```{eval-rst}
.. autoclass:: aws.osml.io.GraphicsAssetProvider
   :members:
   :undoc-members:
   :show-inheritance:
```

## DataAssetProvider

Returned by {meth}`~aws.osml.io.DatasetReader.get_asset` for structured data
assets. Provides XML and JSON parsing methods.

```{eval-rst}
.. autoclass:: aws.osml.io.DataAssetProvider
   :members:
   :undoc-members:
   :show-inheritance:
```
