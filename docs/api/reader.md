# DatasetReader

A `DatasetReader` provides read access to a geospatial dataset and its assets.

Calling {meth}`~aws.osml.io.DatasetReader.get_asset` returns a specialised
provider whose type matches the asset's category:
{class}`~aws.osml.io.ImageAssetProvider` for images,
{class}`~aws.osml.io.TextAssetProvider` for text,
{class}`~aws.osml.io.DataAssetProvider` for structured data, or
{class}`~aws.osml.io.GraphicsAssetProvider` for vector graphics.

```{eval-rst}
.. autoclass:: aws.osml.io.DatasetReader
   :members:
   :undoc-members:
   :show-inheritance:
```
