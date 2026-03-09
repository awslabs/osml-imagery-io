# TRE and DES Structure Limitations

This document catalogs limitations, simplifications, and missing specifications in the Kaitai Struct definitions under `data/structures/`.

## Summary

| Category | Count | Description |
|----------|-------|-------------|
| Stub Definitions | 10 | Raw data capture only, specification not publicly available |
| Simplified Definitions | 4 | Fixed headers parsed, conditional fields captured as raw bytes |
| Semantic Limitations | 1 | Fields parsed but interpretation requires external spec |
| Reference-Only Files | 4 | Documentation placeholders, no parseable structure |
| Missing STDI-0001 TREs | 2 | Referenced but not implemented |
| Legacy/Inactive | 1 | Superseded by newer TRE |
| Unpublished Specification | 1 | TRE registered but spec not yet published |

---

## Stub Definitions (MIE4NITF)

These TREs are part of the Motion Imagery Extensions for NITF 2.1 (MIE4NITF) specification defined in NGA.STND.0044. The full field specifications are in NGA.STND.0044_1.3, which is not publicly available in STDI-0002. Raw data is preserved for round-trip fidelity.

| TRE | File | Purpose |
|-----|------|---------|
| CAMSDA | `tre_camsda.ksy` | Camera Set Definition - defines camera sets and positions on NCCS |
| FASYWA | `tre_fasywa.ksy` | Frame Asynchronous Wrapper - wraps TREs with time association |
| FREESA | `tre_freesa.ksy` | Free Space - placeholder to reserve space (no data allowed) |
| FSYNWA | `tre_fsynwa.ksy` | Frame Synchronization Wrapper - wraps TREs with frame association |
| MICIDA | `tre_micida.ksy` | Motion Imagery Collection ID - collection identification |
| MIMCSA | `tre_mimcsa.ksy` | Motion Imagery Collection Summary - frame rate and encoding metadata |
| MTIMFA | `tre_mtimfa.ksy` | Motion Imagery Temporal Block Mapping - temporal block definitions |
| MTIMSA | `tre_mtimsa.ksy` | Motion Imagery Timing - frame timing and timestamps |
| TMINTA | `tre_tminta.ksy` | Time Interval - defines start/end times for time intervals |

**Impact**: These TREs can be read and written as opaque blobs but individual fields cannot be accessed or validated.

**Resolution**: Obtain NGA.STND.0044_1.3 specification and implement full field parsing.

---

## Simplified Definitions (Complex Conditional Logic)

These TREs have complex conditional logic based on existence masks or method selectors that requires runtime bitwise evaluation. Fixed header fields are fully parsed; conditional/variable sections are captured as raw bytes.

| TRE | File | Limitation |
|-----|------|------------|
| BANDSB | `tre_bandsb.ksy` | 32-bit existence mask controls per-band conditional fields |
| BCHIPA | `tre_bchipa.ksy` | Include flags (A, B, C) control nested conditional sections with parent references |
| ILLUMB | `tre_illumb.ksy` | 24-bit existence mask controls illumination condition fields |
| IOMAPA | `tre_iomapa.ksy` | MAP_SELECT value determines method-specific data structure (4 variants) |

**Impact**: Header fields accessible; conditional data requires additional parsing logic in application code.

**Resolution**: Implement runtime conditional parsing in Rust code using the captured raw bytes and existence mask values.

---

## Semantic Limitations (External Specification Required)

These TREs are fully parsed at the field level, but correct interpretation of the data requires information from an external specification.

| TRE | File | Limitation |
|-----|------|------------|
| RPC00A | `tre_rpc00a.ksy` | Polynomial term order differs from RPC00B; exact order defined in STDI-0001 (not publicly available) |

**Details**: RPC00A and RPC00B have identical field layouts (sizes, types), but the 20 polynomial coefficients use different term orderings. Without STDI-0001, the coefficients cannot be correctly applied to the rational polynomial model.

**Impact**: Fields can be read/written correctly, but geolocation calculations using RPC00A coefficients will produce incorrect results without the term order mapping.

**Resolution**: Obtain STDI-0001 for the RPC00A term order specification, or use RPC00B which has documented term order in STDI-0002.

---

## Reference-Only Files (No Parseable Structure)

These files document TRE/DES collections or external specifications. They do not define a single parseable structure.

| Name | File | Reason |
|------|------|--------|
| NSDE | `tre_nsde.ksy` | Collection of TREs defined in STDI-0001 (individual TREs like STDIDC have their own files) |
| NCDRD | `tre_ncdrd.ksy` | Reference to STDI-0006 commercial dataset requirements (TREs defined elsewhere) |
| DPPDB | `tre_dppdb.ksy` | Reference to MIL-PRF-89034 Digital Point Positioning Data Base (TREs not publicly available) |
| WBRD_Frame | `des_wbrd_frame.ksy` | Wideband Radar Frame DES - specification not publicly available (contact NTB) |

**Impact**: These files serve as documentation only. Referenced TREs must be obtained from external specifications.

**Resolution**: 
- NSDE: Individual TREs already implemented (e.g., `tre_stdidc.ksy`)
- NCDRD: Many TREs already implemented; remaining ones in STDI-0006
- DPPDB: Obtain MIL-PRF-89034 for TRE definitions
- WBRD_Frame: Contact NTB at ntbchair@nga.mil for specification

---

## Unpublished Specification

| TRE | File | Status |
|-----|------|--------|
| PIVECA | `tre_piveca.ksy` | Approved for registration 2019-02-28, but full specification marked "To Be Determined" in STDI-0002 |

**Impact**: TRE can be read/written as raw bytes only. Field structure unknown.

**Resolution**: Monitor STDI-0002 updates for published specification.

---

## Missing STDI-0001 TREs

These TREs are referenced in the NSDE collection (STDI-0001) but have not been implemented. STDI-0001 is not publicly available.

| TRE | Description | Status |
|-----|-------------|--------|
| STDIDA | Standard ID Extension A | Not implemented |
| STDIDB | Standard ID Extension B | Not implemented |

**Note**: STDIDC (Standard ID Extension) from STDI-0001 is implemented in `tre_stdidc.ksy`.

**Impact**: Files containing STDIDA or STDIDB TREs can still be read (as unknown TREs with raw data), but field-level access is not available.

**Resolution**: Obtain STDI-0001 specification to implement these TREs if needed.

---

## Legacy/Inactive TREs

| TRE | File | Status | Replacement |
|-----|------|--------|-------------|
| BANDSA | `tre_bandsa.ksy` | Inactive since 2007-08-01 | Use BANDSB instead |

**Impact**: Fully implemented for legacy file support, but should not be used for new files.

**Resolution**: None needed - retained for backward compatibility.

---

## DES Notes

### Fully Implemented DES

The following DES definitions are complete:
- CSATTA - Coordinate System Attitude Data
- CSSHPA - Coordinate System Shapefile (Version A)
- CSSHPB - Coordinate System Shapefile (Version B)
- EXT_DEF_CONTENT - External Definition Content
- LIDARA - LiDAR Data
- MRGXMA - Merge XML Metadata
- TRE_OVERFLOW - TRE Overflow
- WEATHER_DATA - Weather/METOC Data
- XML_DATA_CONTENT - XML Data Content

### DES Reference Files

| Name | File | Notes |
|------|------|-------|
| NCDRD | `des_ncdrd.ksy` | Reference to STDI-0006, not a DES type itself |

---

## Recommendations

1. **Priority 1**: Implement runtime conditional parsing for BANDSB, ILLUMB, BCHIPA, and IOMAPA in Rust code
2. **Priority 2**: Obtain NGA.STND.0044_1.3 for MIE4NITF TRE implementations
3. **Priority 3**: Obtain STDI-0001 for RPC00A term order and STDIDA/STDIDB definitions
4. **Priority 4**: Contact NTB for WBRD_Frame and DPPDB specifications if needed
5. **Monitor**: STDI-0002 updates for PIVECA specification publication
