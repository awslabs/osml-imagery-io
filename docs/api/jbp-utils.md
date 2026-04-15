# JBP Utilities

Adapter classes for parsing and formatting NITF metadata field values. These
utilities convert between raw NITF string representations and structured Python
objects.

```{note}
These adapters work with the string values returned by
{meth}`MetadataProvider.as_dict <aws.osml.io.MetadataProvider.as_dict>`. They
do not read files directly — use {class}`~aws.osml.io.IO` to open a dataset
first, then pass metadata values to the appropriate adapter.
```

## DateTimeAdapter

```{eval-rst}
.. autoclass:: aws.osml.io.jbp.utils.DateTimeAdapter
   :members:
   :undoc-members:
   :show-inheritance:
```

## NitfDateTime

```{eval-rst}
.. autoclass:: aws.osml.io.jbp.utils.NitfDateTime
   :members:
   :undoc-members:
   :show-inheritance:
```

## IGEOLOAdapter

```{eval-rst}
.. autoclass:: aws.osml.io.jbp.utils.IGEOLOAdapter
   :members:
   :undoc-members:
   :show-inheritance:
```

## UTMCoordinate

```{eval-rst}
.. autoclass:: aws.osml.io.jbp.utils.UTMCoordinate
   :members:
   :undoc-members:
   :show-inheritance:
```

## SecurityClassificationAdapter

```{eval-rst}
.. autoclass:: aws.osml.io.jbp.utils.SecurityClassificationAdapter
   :members:
   :undoc-members:
   :show-inheritance:
```

## SecurityClassification

```{eval-rst}
.. autoclass:: aws.osml.io.jbp.utils.SecurityClassification
   :members:
   :undoc-members:
   :show-inheritance:
```

## TGTIDAdapter

```{eval-rst}
.. autoclass:: aws.osml.io.jbp.utils.TGTIDAdapter
   :members:
   :undoc-members:
   :show-inheritance:
```

## TGTID

```{eval-rst}
.. autoclass:: aws.osml.io.jbp.utils.TGTID
   :members:
   :undoc-members:
   :show-inheritance:
```

## LocationAdapter

```{eval-rst}
.. autoclass:: aws.osml.io.jbp.utils.LocationAdapter
   :members:
   :undoc-members:
   :show-inheritance:
```
