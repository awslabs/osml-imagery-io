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
  SDE information.

  Section B (INCLUDE_B == "Y"): Original band information including per-band
  LUT data. Each original band carries NLUTS_ORIGn LUTs; when NLUTS_ORIGn != 0
  the band adds NELUT_ORIGn (entries-per-LUT) followed by NLUTS_ORIGn x
  NELUT_ORIGn one-byte LUT entries (LUTD_ORIGnmp).

  Section C (INCLUDE_C == "Y"): Band correspondence/mapping information. Each
  current band carries a variable-length SEMANTIC_MEANINGn (size =
  SEMANTIC_SIZEn) and, when NUM_ORIG_BANDSn != 0, a per-original-band loop; the
  WEIGHTnm value is present only when MAPPING_TYPEn == "WEIGHTED", and a
  FORMULA_SIZEn / FORMULAn pair is present only when MAPPING_TYPEn ==
  "FORMULAIC".

  Because INCLUDE_B and INCLUDE_C each land AFTER the entire preceding
  conditional block, all three sections are modeled field-by-field so the flags
  decode at their correct offsets (a raw-bytes collapse of Sections B/C would
  otherwise hoist the flags into the middle of the preceding block).

  Multiple instances may be required to contain all support data for a
  band-wise processed image.

  Variable length TRE (minimum 68 bytes, maximum 99985 bytes)

  Reference: STDI-0002 Volume 1, Appendix AR - BCHIPA
  (Table AR.5-3, pp. AR-42 to AR-60)

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

  # --- Section A: Image segment identification and SDE info ---
  # Gated by INCLUDE_A == "Y". Per Table AR.5-3, INCLUDE_B appears only after
  # the entire INCLUDE_A conditional block, so Section A is modeled in full.

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
      Image Display Level (IDLVL) of each original image segment comprising
      the band-wise processed image. Repeated NUM_BWP_IS times.
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
      O = original, W = wrapped original, C = current, D = potentially
      discrepant, U = unknown, N = not applicable. 1 BCS-A.

  # --- INCLUDE_B: Original band information ---
  # Lands after the full Section A block, per Table AR.5-3 (p. AR-50).
  - id: INCLUDE_B
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Inclusion flag for original band information.
      Y = include fields, N = exclude fields.

  - id: NUM_ORIGINAL_BANDS
    type: str
    size: 5
    encoding: BCS-N
    if: "INCLUDE_B == \"Y\""
    doc: |
      Number of original image bands reported in this BCHIPA TRE instance.
      Range: 00001-99999.

  - id: ORIGINAL_BANDS
    type: original_band_t
    repeat: expr
    repeat-expr: NUM_ORIGINAL_BANDS.to_i
    if: "INCLUDE_B == \"Y\""
    doc: |
      Per-original-band support data. Repeated NUM_ORIGINAL_BANDS times.

  # --- INCLUDE_C: Band correspondence information ---
  # Lands after the full Section B block, per Table AR.5-3 (p. AR-55).
  - id: INCLUDE_C
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Inclusion flag for band correspondence information.
      Y = include fields, N = exclude fields.

  - id: NUM_CURR_BANDS
    type: str
    size: 5
    encoding: BCS-N
    if: "INCLUDE_C == \"Y\""
    doc: |
      Number of current image bands reported in this BCHIPA TRE instance.
      Range: 00001-99999.

  - id: CURRENT_BANDS
    type: current_band_t
    repeat: expr
    repeat-expr: NUM_CURR_BANDS.to_i
    if: "INCLUDE_C == \"Y\""
    doc: |
      Per-current-band mapping data. Repeated NUM_CURR_BANDS times.

types:
  # --- Section B: original-band record (Table AR.5-3, pp. AR-51 to AR-54) ---
  original_band_t:
    seq:
      - id: ORIG_BAND_NUMBER
        type: str
        size: 5
        encoding: BCS-N
        doc: "Band number of this original image band. Range: 00001-99999."

      - id: IREPBAND_ORIG
        type: str
        size: 2
        encoding: BCS-A
        doc: |
          Original image band representation (IREPBANDn). LU, R, G, B, M, Y,
          Cb, Cr, or all BCS blank spaces.

      - id: ISUBCAT_ORIG
        type: str
        size: 8
        encoding: BCS-A
        doc: |
          Original image band subcategory (ISUBCATn), 8 octets (the standard
          6-octet ISUBCAT plus an optional ".M" motion-imagery suffix).

      - id: IFC_ORIG
        type: str
        size: 1
        encoding: BCS-A
        doc: "Original image band image filter condition. N (none)."

      - id: IMFLT_ORIG
        type: str
        size: 3
        encoding: BCS-A
        doc: "Original image band standard image filter code. All blank spaces."

      - id: NLUTS_ORIG
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Number of LUTs associated with this original band. Range 0-4
          (value 4 reserved). BCS zero (0x30) when no LUTs are included.

      - id: NELUT_ORIG
        type: str
        size: 5
        encoding: BCS-N
        if: NLUTS_ORIG.to_i != 0
        doc: |
          Number of LUT entries in each of the LUTs for this original band.
          Range 00001-65536. Omitted when NLUTS_ORIG is BCS zero.

      - id: LUTD_ORIG
        type: u1
        repeat: expr
        repeat-expr: NLUTS_ORIG.to_i * NELUT_ORIG.to_i
        if: NLUTS_ORIG.to_i != 0
        doc: |
          LUT data entries, ordered by LUT (m = 1..NLUTS_ORIG) then entry
          (p = 1..NELUT_ORIG). Each entry is one 8-bit unsigned value (0-255).
          Total NLUTS_ORIG x NELUT_ORIG entries. Omitted when NLUTS_ORIG is 0.

  # --- Section C: current-band record (Table AR.5-3, pp. AR-56 to AR-60) ---
  current_band_t:
    seq:
      - id: CURR_BAND_NUMBER
        type: str
        size: 5
        encoding: BCS-N
        doc: "Band number of this current image band. Range: 00001-99999."

      - id: SEMANTIC_SIZE
        type: str
        size: 4
        encoding: BCS-N
        doc: |
          Size in octets of the SEMANTIC_MEANING string for this current band.
          Range 0001-9999, or 0000 when no semantic description is provided.

      - id: SEMANTIC_MEANING
        type: str
        size: SEMANTIC_SIZE.to_i
        encoding: BCS-A
        doc: |
          Semantic meaning of this current band (user-defined free text).
          Width = SEMANTIC_SIZE octets; absent when SEMANTIC_SIZE is 0000.

      - id: NUM_ORIG_BANDS
        type: str
        size: 5
        encoding: BCS-N
        doc: |
          Number of original image bands used to create this current band.
          Range 00001-99999, or 00000 when the current band was not derived
          solely from bands in the original image.

      - id: MAPPING_TYPE
        type: str
        size: 15
        encoding: BCS-A
        doc: |
          Type of mapping/data source used to create this current band.
          NTB-controlled values include IDENTICAL, AVERAGE, WEIGHTED,
          LUT_APPLIED, COLOR_MAP, INSERTION, MOSAIC, DEMOSAIC, FORMULAIC,
          or all blank spaces.

      - id: ORIG_BAND_MAPPINGS
        type: orig_band_mapping_t
        repeat: expr
        repeat-expr: NUM_ORIG_BANDS.to_i
        if: NUM_ORIG_BANDS.to_i != 0
        doc: |
          Per-original-band mapping records. Repeated NUM_ORIG_BANDS times.
          Present only when NUM_ORIG_BANDS != 00000.

      - id: FORMULA_SIZE
        type: str
        size: 3
        encoding: BCS-N
        if: MAPPING_TYPE.strip == "FORMULAIC"
        doc: |
          Size in octets of the FORMULA string. Range 001-999.
          Present only when MAPPING_TYPE == "FORMULAIC".

      - id: FORMULA
        type: str
        size: FORMULA_SIZE.to_i
        encoding: BCS-A
        if: MAPPING_TYPE.strip == "FORMULAIC"
        doc: |
          Formula used to create this current band (user-defined free text, or
          NOT_AVAILABLE). Width = FORMULA_SIZE octets. Present only when
          MAPPING_TYPE == "FORMULAIC".

  orig_band_mapping_t:
    seq:
      - id: ORIG_BND_NUM
        type: str
        size: 5
        encoding: BCS-N
        doc: |
          Band number of the original image band used to create the current
          band. Range: 00001-99999.

      - id: WEIGHT
        type: str
        size: 21
        encoding: BCS-A
        if: MAPPING_TYPE.strip == "WEIGHTED"
        doc: |
          Numeric weight applied to this original band, as a 21-character BCS
          scientific-notation value (+-9.99999999999999E+-99). Present only
          when the current band's MAPPING_TYPE == "WEIGHTED".
