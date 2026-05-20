# Integration / Validation Test Data

This directory contains third-party test data used for integration and validation testing.

## Contents

This data includes both valid and invalid imagery samples used to validate the library's behavior against real-world and edge-case inputs.

## External Data Sources

The following external sources provide test imagery that can be used for integration testing:

- [USGS EarthExplorer](https://earthexplorer.usgs.gov/) — Access to open geospatial data sets including DTED versions of SRTM elevation data and NAIP aerial imagery in both JPEG 2000 (compressed) and GeoTIFF (uncompressed) formats.
- [JITC NITF Test Data](https://jitc.fhu.disa.mil/projects/nitf/testdata.aspx) — Official NITF conformance test suite from the Joint Interoperability Test Command. Includes QuickLook positive, transitional, and negative test cases covering format compliance, geospatial metadata, security markings, temporal fields, image segment blocking modes, DES/TRE extensions, and symbol/text segments for NITF 2.1 and NSIF 1.0.
- [codice/imaging-nitf](https://github.com/codice/imaging-nitf/tree/master/shared-test-resources/src/main/resources) — JITC NITF 2.0/2.1 samples, JPEG 2000 conformance files, ECRG, GDAL/NITRO/OSGEO/VTS test data, and multi-resolution imagery.
- [OSGeo/gdal autotest](https://github.com/OSGeo/gdal/tree/master/autotest/gdrivers/data) — GDAL driver test data covering a wide range of raster formats (GeoTIFF, JPEG 2000, NITF, PNG, etc.).


## Usage

Integration tests are **manifest-driven**. A `manifest.yaml` in this directory declares
which files to test, what tags to apply, and what outcome to expect (success or a specific
exception). See `tests/integration/__init__.py` for the authoritative `IntegrationEntry`
dataclass definition.

Example `manifest.yaml`:

```yaml
version: "1.0.0"

entries:
  # Positive test — file should open and pass all checks
  - path: "source/category/sample_image.ntf"
    label: "GOOD-SAMPLE-01"
    description: "Uncompressed 3-band RGB NITF"
    tags: ["format"]

  # Negative test — file should raise the specified exception
  - path: "source/category/bad_image.ntf"
    label: "BAD-SAMPLE-01"
    description: "Corrupt file header (unsupported version)"
    tags: ["format"]
    expected_exception: "Exception"
    expected_message: "Invalid magic number"
```

Each entry specifies:
- `path` — relative path from this directory to the test file
- `label` — short identifier shown in test output
- `description` — human-readable summary
- `tags` — list of strings used for filtering (e.g. `format`, `sicd`, `slow`)
- `expected_exception` / `expected_message` — for negative tests that should raise

To run integration tests:

```bash
pytest tests/integration/ -m integration

# Filter by tag
pytest tests/integration/ -m integration --include-tags sicd,sidd
pytest tests/integration/ -m integration --exclude-tags slow
```

Tests skip gracefully when this directory is empty or when `manifest.yaml` is missing.

## Updating the Manifest

To add new files to the manifest without manually editing YAML, use `--update-manifest`.
This discovers all imagery files in the data directory (recursively), opens each one to
verify readability, and appends new entries to `manifest.yaml`. Existing entries are
preserved unchanged.

```bash
pytest tests/integration/ -m integration --update-manifest
```

Auto-discovered entries are tagged by format (e.g. `nitf`, `tiff`, `dted`) and flagged
with `slow` if opening takes longer than 5 seconds. Files that fail to open are still
added as positive test entries — they will fail when the test suite runs. If the file 
is a legitimate negative test you should provide the expected_exception and expected_message
entries.


## Environment Override

If your test data already resides in a different location, set `OSML_IO_INTEGRATION_DATA`
to provide an alternate root directory. The `path` values in `manifest.yaml` are resolved
relative to this root instead of the default `data/integration/` directory.

```bash
export OSML_IO_INTEGRATION_DATA=/path/to/your/data
```
