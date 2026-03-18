# Cloud Optimized GeoTIFF (COG) Roadmap

This roadmap covers Cloud Optimized GeoTIFF support in osml-imagery-io. COG is a tiled GeoTIFF with overviews arranged in a specific IFD layout for efficient HTTP range-request access. It builds on two prerequisites:

- **{doc}`tiff`** — Basic GeoTIFF reading and writing (libtiff FFI, tile I/O, GeoTIFF metadata)
- **{doc}`image-pyramid`** — Cross-format image pyramid support (overview IFD navigation, reduced-resolution access, overview generation)

COG reading and writing are separated into two phases. Both assume the TIFF roadmap (Phases 1–3) and the image pyramid roadmap are complete.

## Phase 1: COG Reading

**Objective**: Read COG files with efficient tile access and overview navigation.

**Scope**:
- COG validation: verify file meets COG requirements (tiled, overviews present, IFD order correct, ghost metadata)
- Efficient tile access: COG files are tiled GeoTIFFs with overviews — the image pyramid support ({doc}`image-pyramid`) handles the core multi-resolution access pattern
- BigTIFF support: COG files larger than 4 GB use BigTIFF format — libtiff handles this transparently when opened via `TIFFClientOpen`
- JPEG and JPEG2000 tile compression: COG tiles may use JPEG (compression=7) or JPEG2000 — libtiff handles JPEG natively; for JPEG2000 tiles, delegate to our existing OpenJPEG codec
- Remote COG access is not handled here — the IO layer is responsible for producing the `&[u8]` byte slice (via future mmap or S3-backed mmap). The TIFF format implementation is unaware of whether the bytes are local or remote.

**Tasks**:
- [ ] Add COG detection (check IFD layout, ghost metadata)
- [ ] Add BigTIFF support in `TIFFClientOpen` calls
- [ ] Verify JPEG-compressed tile reading works through libtiff
- [ ] Add integration tests with real COG files
- [ ] Document COG-specific metadata fields

## Phase 2: COG Writing

**Objective**: Write valid COG files that conform to the COG specification.

**Scope**:
- COG layout requirements:
  - Ghost IFD with COG metadata
  - Full-resolution IFD first, then overviews in descending resolution order
  - All IFDs tiled
  - Tile data ordered for sequential access
- Encoding hints:
  - `"Format"` → `"COG"` (triggers COG-specific layout)
  - `"Compression"` → `"Deflate"`, `"LZW"`, `"JPEG"` (default: `"Deflate"`)
  - `"JPEGQuality"` → `"75"` (for JPEG compression)
  - `"Overviews"` → `"true"` (required for COG, auto-enabled)
- Overview generation uses the image pyramid writer support ({doc}`image-pyramid`)
- Validate output meets COG spec before finalizing

**Tasks**:
- [ ] Implement COG-specific IFD ordering in writer
- [ ] Add ghost IFD generation
- [ ] Add tile data ordering for sequential access
- [ ] Add COG validation pass
- [ ] Add unit and integration tests for COG output

## Testing Plan

### Unit Tests (Python)

- `tests/test_tiff_cog.py` — COG detection, overview access, COG-specific metadata

### Integration Tests

- Test with real-world COG files from public sources (Landsat, Sentinel-2 COGs)
- Marker: `pytest -m integration` to run

## Reference Materials

- **OGC Cloud Optimized GeoTIFF Standard** (`reference-materials/GeoTIFF/OGCCloudOptimizedGeoTIFFStandard.pdf`, 34 pages) — IFD ordering requirements, ghost metadata, tiling constraints
- **TIFF 6.0** (`reference-materials/GeoTIFF/TIFF6.pdf`) — `NewSubfileType` tag, IFD structure, BigTIFF
- **OGC GeoTIFF Standard** (`reference-materials/GeoTIFF/OGCGeoTIFFStandard.pdf`) — GeoKey metadata carried through to COG files
