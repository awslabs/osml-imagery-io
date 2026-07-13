# Parser Infrastructure

## StructureRegistry

```{eval-rst}
.. autoclass:: aws.osml.io.StructureRegistry
   :members:
   :undoc-members:
   :show-inheritance:
```

## StructureDefinition

A `StructureDefinition` is the entire public interface for reading and writing a
named binary structure. Its `decode` method parses raw bytes into a nested dict
(lists for repeated fields, dicts for nested types), and its `encode` method
serializes such a dict back to bytes — a symmetric, dict-based round trip.

```{eval-rst}
.. autoclass:: aws.osml.io.StructureDefinition
   :members:
   :undoc-members:
   :show-inheritance:
```
