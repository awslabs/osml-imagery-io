# Implementation Plan: TRE and DES Support

## Overview

This plan implements Phase 3 of the JBP project: Tagged Record Extensions (TREs) and Data Extension Segments (DES) support. The implementation extends the existing JBP reader/writer to parse TRE metadata and expose it through the unified MetadataProvider interface.

## Tasks

- [-] 1. Implement TreEnvelope struct and parsing
  - [x] 1.1 Create `src/jbp/tre.rs` module with TreEnvelope struct
    - Define TreEnvelope with tag (String) and data (Vec<u8>) fields
    - Implement `parse()` to extract single envelope from bytes (CETAG + CEL + CEDATA)
    - Implement `parse_all()` to extract all envelopes from a byte slice
    - Implement `to_bytes()` for serialization
    - Implement `envelope_size()` helper
    - _Requirements: 1.1, 1.2, 1.3, 9.1, 9.2, 9.3, 9.4_
  
  - [x] 1.2 Write property test for TRE envelope round-trip
    - **Property 1: TRE Envelope Round-Trip**
    - **Validates: Requirements 1.1, 1.2, 1.3, 9.1, 9.2, 9.3, 9.4, 17.1**
  
  - [x] 1.3 Implement CETAG validation
    - Validate 6-character alphanumeric format
    - Return InvalidCetag error for invalid tags
    - _Requirements: 1.4, 16.1_
  
  - [x] 1.4 Implement CEL/CEDATA length validation
    - Validate CEL matches actual CEDATA length
    - Return LengthMismatch error on mismatch
    - Return UnexpectedEof if insufficient bytes
    - _Requirements: 1.5, 16.2_

- [x] 2. Checkpoint - Ensure TreEnvelope tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 3. Implement tre_fields module
  - [x] 3.1 Create `src/jbp/tre_fields.rs` module
    - Implement `create_accessor()` to create StructureAccessor for TRE CEDATA
    - Lookup definition from registry using `TRE_{CETAG}` pattern
    - Return None if no definition exists (unknown TRE)
    - _Requirements: 2.1, 2.2, 14.1, 14.2_
  
  - [x] 3.2 Implement `has_definition()` helper
    - Check if TRE definition exists in registry
    - _Requirements: 2.3, 14.5_
  
  - [x] 3.3 Write property test for known TRE field extraction
    - **Property 3: Known TRE Field Extraction**
    - **Validates: Requirements 2.1, 2.2, 2.4, 2.5**

- [x] 4. Implement overflow module
  - [x] 4.1 Create `src/jbp/overflow.rs` module
    - Implement `get_image_overflow_indices()` to extract UDOFL and IXSOFL from image subheader
    - Implement similar functions for graphic, text, and file header overflow fields
    - _Requirements: 6.3, 6.4, 6.5, 6.6_
  
  - [x] 4.2 Implement `fetch_overflow_tres()`
    - Accept 1-based DES index, DES locations, and file data
    - Return empty vec if index is 0
    - Parse TRE envelopes from DES data section
    - Return InvalidOverflowIndex error if index exceeds DES count
    - _Requirements: 6.1, 6.2_
  
  - [x] 4.3 Write property test for overflow resolution
    - **Property 5: TRE Overflow Resolution via Index**
    - **Validates: Requirements 6.1, 6.2, 6.3, 6.4, 6.5, 6.6**

- [x] 5. Checkpoint - Ensure overflow module tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 6. Extend JBPSegmentMetadataProvider with TRE support
  - [x] 6.1 Add TRE fields to JBPSegmentMetadataProvider
    - Add `tre_envelopes: Vec<TreEnvelope>` field
    - Add `registry: Arc<StructureRegistry>` field
    - Create `with_tres()` constructor
    - _Requirements: 18.1, 18.2_
  
  - [x] 6.2 Update `as_dict()` to include TRE fields
    - Iterate over tre_envelopes
    - Create accessor for each known TRE
    - Add fields with CETAG prefix (e.g., "GEOLOB.ARV")
    - Apply prefix filtering for TRE fields
    - Skip unknown TREs in metadata output
    - _Requirements: 18.3, 18.4_
  
  - [x] 6.3 Write property test for metadata TRE access
    - **Property 7: Metadata Interface TRE Access**
    - **Validates: Requirements 18.1, 18.2, 18.3, 18.4**

- [x] 7. Integrate TRE parsing into JBPDatasetReader
  - [x] 7.1 Parse TREs from segment subheaders
    - Extract TRE bytes from UDID, IXSHD fields in image subheader
    - Extract TRE bytes from SXSHD field in graphic subheader
    - Extract TRE bytes from TXSHD field in text subheader
    - Parse TRE envelopes using TreEnvelope::parse_all()
    - _Requirements: 3.3, 3.4, 3.5, 3.6_
  
  - [x] 7.2 Resolve overflow TREs
    - Check overflow index fields (UDOFL, IXSOFL, etc.)
    - Fetch overflow TREs using overflow::fetch_overflow_tres()
    - Merge inline and overflow TREs
    - _Requirements: 6.1, 6.2_
  
  - [x] 7.3 Create metadata providers with TRE data
    - Pass TRE envelopes to JBPSegmentMetadataProvider::with_tres()
    - Pass registry reference for TRE definition lookup
    - _Requirements: 18.1, 18.2_
  
  - [x] 7.4 Write property test for TRE location extraction
    - **Property 6: TRE Location Extraction**
    - **Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7**

- [x] 8. Checkpoint - Ensure reader integration tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 9. Implement TRE writing support
  - [x] 9.1 Implement `write_tre_envelopes()` function
    - Serialize list of TreEnvelope to bytes
    - Concatenate all envelope bytes
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5_
  
  - [x] 9.2 Implement OverflowSource enum
    - Define variants for each header field type
    - Implement `to_desoflw()` to convert to 6-char DESOFLW value
    - _Requirements: 12.3, 12.4_
  
  - [x] 9.3 Implement `create_overflow_des()` function
    - Create TRE_OVERFLOW DES subheader with DESOFLW and DESITEM
    - Serialize overflow TREs as DES data
    - Return (subheader_bytes, data_bytes)
    - _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5_

- [x] 10. Integrate TRE writing into JBPDatasetWriter
  - [x] 10.1 Accept TRE data in metadata
    - Parse TRE field values from metadata with CETAG prefix
    - Group fields by CETAG
    - _Requirements: 18.6_
  
  - [x] 10.2 Serialize TREs to segment headers
    - Write TREs to appropriate header fields (UDID, IXSHD, etc.)
    - Calculate and set length fields (UDIDL, IXSHDL, etc.)
    - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5, 10.6_
  
  - [x] 10.3 Handle TRE overflow
    - Check if TREs exceed header field limits
    - Create TRE_OVERFLOW DES for excess TREs
    - Set overflow index fields (UDOFL, IXSOFL, etc.)
    - _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5_
  
  - [x] 10.4 Write property test for unknown TRE preservation
    - **Property 2: Unknown TRE Preservation**
    - **Validates: Requirements 2.3, 4.1, 4.2, 4.3, 17.3**
  
  - [x] 10.5 Write property test for TRE field value round-trip
    - **Property 4: TRE Field Value Round-Trip**
    - **Validates: Requirements 8.1, 8.2, 8.3, 17.2**

- [x] 11. Checkpoint - Ensure writer integration tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 12. Create directory structure and core TRE definitions
  - [x] 12.1 Create `data/structures/tre/` and `data/structures/des/` directories
    - _Requirements: 13.1, 15.1_
  
  - [x] 12.2 Create GEOLOB TRE definition (`tre_geolob.ksy`)
    - Reference: Vol-1-App P - GEOSDE.pdf
    - Define fields: ARV, BRV, LSO, PSO
    - _Requirements: 19.1_
  
  - [x] 12.3 Create GEOPSB TRE definition (`tre_geopsb.ksy`)
    - Reference: Vol-1-App P - GEOSDE.pdf
    - _Requirements: 19.1_
  
  - [x] 12.4 Create PRJPSB TRE definition (`tre_prjpsb.ksy`)
    - Reference: Vol-1-App P - GEOSDE.pdf
    - _Requirements: 19.1_
  
  - [x] 12.5 Create MAPLOB TRE definition (`tre_maplob.ksy`)
    - Reference: Vol-1-App P - GEOSDE.pdf
    - _Requirements: 19.1_

- [x] 13. Create sensor and RPC TRE definitions
  - [x] 13.1 Create SENSRB TRE definition (`tre_sensrb.ksy`)
    - Reference: Vol-1-App Z - SENSRB.pdf
    - Complex TRE with conditional sections (15 modules)
    - _Requirements: 19.2_
  
  - [x] 13.2 Create SENSRA TRE definition (`tre_sensra.ksy`)
    - Reference: Vol-1-App Z - SENSRB.pdf (Section Z.6.1 SENSRA to SENSRB Mapping)
    - Legacy/inactive TRE, superseded by SENSRB
    - _Requirements: 19.2_
  
  - [x] 13.3 Create CSEXRA TRE definition (`tre_csexra.ksy`)
    - Reference: STDI-0006 (NCDRD) - full spec not publicly available
    - Commercial exploitation reference data TRE
    - _Requirements: 19.2_
  
  - [x] 13.4 Create RPC00A TRE definition (`tre_rpc00a.ksy`)
    - Reference: Vol-1-App E - ASDE.pdf
    - RPC with L,P,H polynomial term order
    - _Requirements: 19.4_
  
  - [x] 13.5 Create RPC00B TRE definition (`tre_rpc00b.ksy`)
    - Reference: Vol-1-App E - ASDE.pdf, Table E-22
    - RPC with X,Y,Z polynomial term order (1041 bytes)
    - _Requirements: 19.4_

- [x] 14. Create image chip and band TRE definitions
  - [x] 14.1 Create ICHIPB TRE definition (`tre_ichipb.ksy`)
    - Reference: Vol-1-App B - ICHIPB.pdf
    - _Requirements: 19.3_
  
  - [x] 14.2 Create BCHIPA TRE definition (`tre_bchipa.ksy`)
    - Reference: Vol-1-App AR - BCHIPA.pdf
    - _Requirements: 19.3_
  
  - [x] 14.3 Create BANDSB TRE definition (`tre_bandsb.ksy`)
    - Reference: Vol-1-App X - BANDSB.pdf
    - Complex TRE with repeated band entries
    - _Requirements: 19.5_
  
  - [x] 14.4 Create BANDSA TRE definition (`tre_bandsa.ksy`)
    - Reference: Vol-1-App X - BANDSB.pdf
    - _Requirements: 19.5_

- [x] 15. Create JPEG 2000 and security TRE definitions
  - [x] 15.1 Create J2KLRA TRE definition (`tre_j2klra.ksy`)
    - Reference: Vol-1-App Y - J2KLRA.pdf
    - _Requirements: 19.6_
  
  - [x] 15.2 Create SECURA TRE definition (`tre_secura.ksy`)
    - Reference: Vol-1-App AI - SECURA.pdf
    - _Requirements: 19.7_

- [x] 16. Create history and comment TRE definitions
  - [x] 16.1 Create HISTOA TRE definition (`tre_histoa.ksy`)
    - Reference: Vol-1-App L - HISTOA.pdf
    - _Requirements: 19.8_
  
  - [x] 16.2 Create COMNTA TRE definition (`tre_comnta.ksy`)
    - Reference: Vol-1-App AU - COMNTA.pdf
    - _Requirements: 19.9_
  
  - [x] 16.3 Create ENGRDA TRE definition (`tre_engrda.ksy`)
    - Reference: Vol-1-App N - ENGRDA.pdf
    - _Requirements: 19.10_

- [x] 17. Create additional Volume 1 TRE definitions (Group A)
  - [x] 17.1 Create PIXQLA TRE definition (`tre_pixqla.ksy`)
    - Reference: Vol-1-App AA - PIXQLA.pdf
  
  - [x] 17.2 Create RELCCA TRE definition (`tre_relcca.ksy`)
    - Reference: Vol-1-App AD - RELCCA.pdf
  
  - [x] 17.3 Create XMLDCA TRE definition (`tre_xmldca.ksy`)
    - Reference: Vol-1-App AE - XMLDCA.pdf
  
  - [x] 17.4 Create CCINFA TRE definition (`tre_ccinfa.ksy`)
    - Reference: Vol-1-App AG - CCINFA.pdf
  
  - [x] 17.5 Create PIXMTA TRE definition (`tre_pixmta.ksy`)
    - Reference: Vol-1-App AJ - PIXMTA.pdf

- [x] 18. Create additional Volume 1 TRE definitions (Group B)
  - [x] 18.1 Create MATESA TRE definition (`tre_matesa.ksy`)
    - Reference: Vol-1-App AK - MATESA.pdf
  
  - [x] 18.2 Create ILLUMA TRE definition (`tre_illuma.ksy`)
    - Reference: Vol-1-App AL - ILLUMA-ILLUMB.pdf
  
  - [x] 18.3 Create ILLUMB TRE definition (`tre_illumb.ksy`)
    - Reference: Vol-1-App AL - ILLUMA-ILLUMB.pdf
  
  - [x] 18.4 Create PIVECA TRE definition (`tre_piveca.ksy`)
    - Reference: Vol-1-App AM - PIVECA.pdf
  
  - [x] 18.5 Create FRMSGA TRE definition (`tre_frmsga.ksy`)
    - Reference: Vol-1-App AN - FRMSGA.pdf

- [x] 19. Create additional Volume 1 TRE definitions (Group C)
  - [x] 19.1 Create SODDXA TRE definition (`tre_soddxa.ksy`)
    - Reference: Vol-1-App AP - SODDXA.pdf
  
  - [x] 19.2 Create ASTORA TRE definition (`tre_astora.ksy`)
    - Reference: Vol-1-App AQ - ASTORA.pdf
  
  - [x] 19.3 Create SYSIDA TRE definition (`tre_sysida.ksy`)
    - Reference: Vol-1-App AS - SYSIDA.pdf
  
  - [x] 19.4 Create S2EVPA TRE definition (`tre_s2evpa.ksy`)
    - Reference: Vol-1-App AT - S2EVPA.pdf

- [x] 20. Create legacy TRE definitions
  - [x] 20.1 Create PIAE TRE definition (`tre_piae.ksy`)
    - Reference: Vol-1-App C - PIAE.pdf
  
  - [x] 20.2 Create CSDE TRE definition (`tre_csde.ksy`)
    - Reference: Vol-1-App D - CSDE.pdf
  
  - [x] 20.3 Create ASDE TRE definition (`tre_asde.ksy`)
    - Reference: Vol-1-App E - ASDE.pdf
  
  - [x] 20.4 Create IOMAPA TRE definition (`tre_iomapa.ksy`)
    - Reference: Vol-1-App F - IOMAPA.pdf
  
  - [x] 20.5 Create NBLOCA TRE definition (`tre_nbloca.ksy`)
    - Reference: Vol-1-App I - NBLOCA.pdf

- [x] 21. Create specialized TRE definitions
  - [x] 21.1 Create MITOCA TRE definition (`tre_mitoca.ksy`)
    - Reference: Vol-1-App O - MITOCA.pdf
  
  - [x] 21.2 Create NSDE TRE definition (`tre_nsde.ksy`)
    - Reference: Vol-1-App R - NSDE.pdf
  
  - [x] 21.3 Create NCDRD TRE definition (`tre_ncdrd.ksy`)
    - Reference: Vol-1-App S - NCDRD.pdf
  
  - [x] 21.4 Create DPPDB TRE definitions (`tre_dppdb*.ksy`)
    - Reference: Vol-1-App V - DPPDB.pdf
    - May include multiple related TREs

- [x] 22. Create RSM TRE definitions
  - [x] 22.1 Create RSM TRE definitions (`tre_rsm*.ksy`)
    - Reference: Vol-1-App U - RSM.pdf
    - Includes RSMIDA, RSMPCA, RSMPIA, RSMECA, RSMGGA, RSMDCA, RSMAPA, RSMGIA

- [x] 23. Create GLAS-GFM TRE definitions
  - [x] 23.1 Create GLAS-GFM TRE definitions (`tre_glas*.ksy`)
    - Reference: Vol-1-App AH - GLAS-GFM.pdf
    - Reference: Vol-2-App M - GLAS-GFM.pdf (DES version)

- [x] 24. Create MIE4NITF TRE definitions
  - [x] 24.1 Create MIE4NITF TRE definitions (`tre_mie4nitf*.ksy`)
    - Reference: Vol-1-App AF - MIE4NITF.pdf
    - Motion imagery extension TREs

- [x] 25. Create DES definition files
  - [x] 25.1 Create TRE_OVERFLOW DES definition (`des_tre_overflow.ksy`)
    - Reference: Vol-2-App A - TRE Overflow.pdf
    - _Requirements: 20.1_
  
  - [x] 25.2 Create XML_DATA_CONTENT DES definition (`des_xml_data_content.ksy`)
    - Reference: Vol-2-App F - XML_DATA_CONTENT.pdf
    - _Requirements: 20.2_
  
  - [x] 25.3 Create CSATTA DES definition (`des_csatta.ksy`)
    - Reference: Vol-2-App C - CSATTA.pdf
    - _Requirements: 20.3_
  
  - [x] 25.4 Create CSSHPA DES definition (`des_csshpa.ksy`)
    - Reference: Vol-2-App D - CSSHPA-CSSHPB.pdf
    - _Requirements: 20.4_
  
  - [x] 25.5 Create CSSHPB DES definition (`des_csshpb.ksy`)
    - Reference: Vol-2-App D - CSSHPA-CSSHPB.pdf
    - _Requirements: 20.4_

- [x] 26. Create additional DES definition files
  - [x] 26.1 Create WBRD_Frame DES definition (`des_wbrd_frame.ksy`)
    - Reference: Vol-2-App E - WBRD_Frame.pdf
  
  - [x] 26.2 Create LIDARA DES definition (`des_lidara.ksy`)
    - Reference: Vol-2-App J - LIDARA.pdf
  
  - [x] 26.3 Create EXT_DEF_CONTENT DES definition (`des_ext_def_content.ksy`)
    - Reference: Vol-2-App K - EXT_DEF_CONTENT.pdf
  
  - [x] 26.4 Create WEATHER_DATA DES definition (`des_weather_data.ksy`)
    - Reference: Vol-2-App L - WEATHER_DATA.pdf
  
  - [x] 26.5 Create MRGXMA DES definition (`des_mrgxma.ksy`)
    - Reference: Vol-2-App O - MRGXMA.pdf
  
  - [x] 26.6 Create NCDRD DES definition (`des_ncdrd.ksy`)
    - Reference: Vol-2-App N - NCDRD.pdf

- [x] 27. Add error types
  - [x] 27.1 Add TRE/DES error variants to CodecError
    - InvalidCetag { tag: String }
    - LengthMismatch { tag: String, cel: usize, actual: usize }
    - InvalidOverflowIndex { index: u16 }
    - TreDefinitionNotFound { name: String }
    - _Requirements: 21.1, 21.2, 21.3, 21.4, 21.5_
  
  - [x] 27.2 Write property test for validation error handling
    - **Property 8: TRE Validation Error Handling**
    - **Validates: Requirements 1.4, 1.5, 16.1, 16.2**

- [x] 28. Final checkpoint - Run full test suite
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional property-based tests
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- TRE definition files (.ksy) will be created based on STDI-0002 specifications
- The implementation extends existing classes rather than creating new ones where possible
