# Roadmap

Future features and enhancements under consideration. Nothing here is committed to a specific release.

- **GPU-accelerated J2K decoding** — Offload JPEG 2000 decode to GPU and deliver pixels directly to device memory
- **Native sub-sampled component access** — Low-level API for accessing individual bands at native resolution without upsampling to the common grid
- **MIE4NITF motion imagery** — Temporal segments, H.264/H.265 codec integration (NGA.STND.0044, IC codes C9/M9, CA/MA)
- **HRE elevation profile** — Elevation-specific metadata, accuracy TREs, and terrain-aware access (basic NITF reading works today)
- **GeoZarr `proj:` and `spatial:` conventions** — CRS, affine transforms, and bounding boxes on the hierarchical tile index
- **Vector Quantization (C4/M4)** — Read-only codebook compression (MIL-STD-188-199). Required for full CLEVEL conformance.
- **ZLIB compression (CC/MC)** — Lossless compression for floating-point scientific data
- **JPEG Lossless (C5/M5)** — Predictive coding + entropy coding, 2–16 bit precision
- **Bi-Level (C1/M1)** — ITU-T T.4 Group 3 fax encoding for 1-bit imagery
- **12-bit JPEG (C3/M3)** — Requires separately compiled `libjpeg12` with `BITS_IN_JSAMPLE=12`
- **SARZip (C7/M7)** — Custom SAR compression (USAF.RDUCE-001). No third-party library available.
- **NITF 2.0 file header** — Different security field structure and field sizes vs 2.1
- **JBP writer improvements** — Additional writer features such as per-band lookup tables (IREP=RGB/LUT)
