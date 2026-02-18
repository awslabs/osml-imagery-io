meta:
  id: tre_iomapa
  title: Input/Output Amplitude Mapping TRE
  endian: be

doc: |
  IOMAPA TRE - Input/Output Amplitude Mapping Extension
  
  Contains data necessary to perform output amplitude mapping process
  for each scan within each image frame. Post-processing applied after
  image data has undergone expansion using 12-bit JPEG/DCT algorithm.
  
  The structure varies based on MAP_SELECT value:
  - Method 0: No mapping (6 bytes CEDATA)
  - Method 1: Lookup table mapping (8202 bytes CEDATA)
  - Method 2: Log mapping (16 bytes CEDATA)
  - Method 3: Polynomial mapping (91 bytes CEDATA)
  
  NOTE: This is a simplified definition that captures the fixed header fields.
  The full TRE uses switch-on for method-specific data which requires runtime
  evaluation. Method-specific data is captured in the method_data field.
  
  Reference: STDI-0002 Volume 1, Appendix F - IOMAPA

seq:
  - id: band_number
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Band Identifier (BAND_NUMBER)
      000 for monochrome or single band imagery.
      3 BCS-N, 000-999.

  - id: map_select
    type: str
    size: 1
    encoding: ASCII
    doc: |
      Mapping Method (MAP_SELECT)
      0=No mapping, 1=Lookup table, 2=Log mapping, 3=Polynomial.
      1 BCS-N, 0-3.

  # Remaining data depends on map_select value
  # This simplified definition captures the raw remaining bytes
  - id: method_data
    size-eos: true
    doc: |
      Method-specific data based on MAP_SELECT value:
      - Method 0: S2 scale factor (2 bytes)
      - Method 1: TABLE_ID, S1, S2, and 4096 output map values (8198 bytes)
      - Method 2: TABLE_ID, S1, S2, R_WHOLE, R_FRACTION (12 bytes)
      - Method 3: TABLE_ID, S1, S2, NO_OF_SEGMENTS, boundaries, and polynomial coefficients (87 bytes)
      Full parsing requires runtime switch-on evaluation.
