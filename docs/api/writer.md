# DatasetWriter

A `DatasetWriter` provides write access to a geospatial dataset.

The {meth}`~aws.osml.io.DatasetWriter.add_asset` method accepts any provider
type: {class}`~aws.osml.io.ImageAssetProvider`,
{class}`~aws.osml.io.BufferedImageAssetProvider`,
{class}`~aws.osml.io.TextAssetProvider`,
{class}`~aws.osml.io.BufferedTextAssetProvider`,
{class}`~aws.osml.io.DataAssetProvider`,
{class}`~aws.osml.io.GraphicsAssetProvider`, or
{class}`~aws.osml.io.AssetProvider` (created via
{meth}`AssetProvider.from_bytes <aws.osml.io.AssetProvider.from_bytes>`).

```{eval-rst}
.. autoclass:: aws.osml.io.DatasetWriter
   :members:
   :undoc-members:
   :show-inheritance:
```
