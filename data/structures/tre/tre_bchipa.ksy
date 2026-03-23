meta:
  id: tre_bchipa
  title: Band Chipping TRE
  endian: be

doc: |
  BCHIPA TRE - Band Chipping Support Data Extension
  
  Records the parsing, reordering, and/or combination of bands that has been
  applied to image data. Provides mapping between current image bands and
  original bands, similar to how ICHIPB provides spatial chipping information.
  
  This is a complex TRE with three conditional sections controlled by
  include flags (INCLUDE_A, INCLUDE_B, INCLUDE_C).
  
  Section A (INCLUDE_A == "Y"): Image segment identification and relevant
  SDE information. Fully parsed with string comparison conditions.
  
  Section B (INCLUDE_B == "Y"): Original band information including per-band
  LUT data with nested conditionals (NLUTS_ORIGn != 0 triggers LUT fields).
  Cannot be individually parsed due to nested conditional logic.
  
  Section C (INCLUDE_C == "Y"): Band correspondence/mapping information with
  variable-length fields (SEMANTIC_MEANINGn size = SEMANTIC_SIZEn) and nested
  conditionals. Cannot be individually parsed due to variable-length fields.
  
  Sections B and C are captured together as raw bytes when present because
  Section B's variable size (due to nested LUT conditionals) prevents
  determining where Section C begins without full runtime parsing.
  
  Multiple instances may be required to contain all support data for a
  band-wise processed image.
  
  Variable length TRE (minimum 68 bytes, maximum 99985 bytes)
  
  Reference: STDI-0002 Volume 1, Appendix AR - BCHIPA

seq:
  - id: SDE_UUID
    type: str
    size: 36
    encoding: BCS-A
    doc: |
      UUID assigned to the series of BCHIPA TREs associated with the image.
      Canonical format using lower-case characters, or all blank spaces (0x20)
      if no UUID is provided.

  - id: NUM_INSTS
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Number of instances of BCHIPA TRE associated with this band-wise
      processed image. Range: 00001-99999

  - id: INSTANCE
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Current instance number within the series of BCHIPA TREs.
      Range: 00001-99999

  - id: INCLUDE_A
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Inclusion flag for image segment identification and relevant SDE info.
      Y = include fields, N = exclude fields.
      Must be Y if INSTANCE = 00001.

  - id: INCLUDE_B
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Inclusion flag for original band information.
      Y = include fields, N = exclude fields.

  - id: INCLUDE_C
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Inclusion flag for band correspondence information.
      Y = include fields, N = exclude fields.

  # --- Section A: Image segment identification and SDE info ---
  # Gated by INCLUDE_A == "Y"

  - id: ISID
    type: str
    size: 10
    encoding: BCS-A
    if: "INCLUDE_A == \"Y\""
    doc: |
      Image segment identifier. IID1 value from the image subheader
      of the band-wise processed image. 10 BCS-A.

  - id: TOT_ORIG_BANDS
    type: str
    size: 5
    encoding: BCS-N
    if: "INCLUDE_A == \"Y\""
    doc: |
      Total number of original bands across all original images that
      contributed to the band-wise processed image. Range: 00001-99999.

  - id: TOT_CURR_BANDS
    type: str
    size: 5
    encoding: BCS-N
    if: "INCLUDE_A == \"Y\""
    doc: |
      Total number of current bands in the band-wise processed image.
      Range: 00001-99999.

  - id: NUM_BWP_IS
    type: str
    size: 3
    encoding: BCS-N
    if: "INCLUDE_A == \"Y\""
    doc: |
      Number of original image segments that contributed bands to the
      band-wise processed image. Range: 001-999.

  - id: BWP_IS
    type: str
    size: 3
    encoding: BCS-N
    repeat: expr
    repeat-expr: NUM_BWP_IS.to_i
    if: "INCLUDE_A == \"Y\""
    doc: |
      Number of bands from each original image segment that were used
      in the band-wise processed image. Repeated NUM_BWP_IS times.
      Range: 001-999 per entry.

  - id: NUM_RLVNT_SDE
    type: str
    size: 3
    encoding: BCS-N
    if: "INCLUDE_A == \"Y\""
    doc: |
      Number of relevant SDEs (Support Data Extensions) associated with
      the band-wise processed image. Range: 000-999.

  - id: SDE_NAME
    type: str
    size: 32
    encoding: BCS-A
    repeat: expr
    repeat-expr: NUM_RLVNT_SDE.to_i
    if: "INCLUDE_A == \"Y\""
    doc: |
      Name of relevant SDE. Repeated NUM_RLVNT_SDE times. 32 BCS-A.

  - id: SDE_STATUS
    type: str
    size: 1
    encoding: BCS-A
    repeat: expr
    repeat-expr: NUM_RLVNT_SDE.to_i
    if: "INCLUDE_A == \"Y\""
    doc: |
      Status of relevant SDE. Repeated NUM_RLVNT_SDE times.
      A = Applicable, N = Not applicable, U = Unknown. 1 BCS-A.

  # --- Sections B and C: Original band info and band correspondence ---
  # Captured as raw bytes because:
  # - Section B has nested conditionals (NLUTS_ORIGn != 0 triggers LUT fields)
  # - Section C has variable-length fields (SEMANTIC_MEANINGn size = SEMANTIC_SIZEn)
  # - Both limitations prevent static KSY parsing
  # - Section B's variable size prevents splitting B and C into separate fields
  - id: SECTION_BC_DATA
    size-eos: true
    doc: |
      Combined Section B and Section C data (raw bytes).
      Present when INCLUDE_B == "Y" and/or INCLUDE_C == "Y".
      
      Section B (if INCLUDE_B == "Y"):
        NUM_ORIGINAL_BANDS (5 BCS-N), then per-band loop:
          ORIG_BAND_NUMBERn (5 BCS-N), IREPBANDn (2 BCS-A),
          ISUBCATn (6 BCS-A), IFCn (1 BCS-A), IMFLTn (3 BCS-A),
          NLUTSn (1 BCS-N), and if NLUTSn != 0:
            NELUTn (5 BCS-N), then NLUTSn x NELUTn LUT entries (1 byte each).
      
      Section C (if INCLUDE_C == "Y"):
        NUM_CURR_BANDS (5 BCS-N), then per-band loop:
          CURR_BAND_NUMBERn (5 BCS-N), SEMANTIC_SIZEn (4 BCS-N),
          SEMANTIC_MEANINGn (variable, size = SEMANTIC_SIZEn),
          NUM_ORIG_BANDSn (5 BCS-N), MAPPING_TYPEn (1 BCS-A),
          and conditional fields based on MAPPING_TYPEn value.
      
      Full parsing requires runtime evaluation of nested conditionals
      and variable-length fields.
