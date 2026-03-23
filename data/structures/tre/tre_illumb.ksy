meta:
  id: tre_illumb
  title: Illumination TRE (Traditional encoding)
  endian: be

doc: |
  ILLUMB TRE - Illumination Tagged Record Extension (Traditional encoding)
  
  Provides illumination metadata using traditional fixed-field encoding.
  Contains information about natural and artificial illumination relevant
  at the time and location of data collection for electro-optical imagery.
  
  Uses a 24-bit EXISTENCE_MASK field (u3) to control which conditional
  fields are present within each illumination set.
  
  Bit definitions (bit 23 = MSB of 24-bit field):
    b23: RAD_QUANTITY (40 ECS-A), RADQ_UNIT (40 ECS-A)
    b22: SUN_AZIMUTHn (5), SUN_ELEVn (5) per set
    b21: MOON_AZIMUTHn (5), MOON_ELEVn (5) per set
    b20: MOON_PHASE_ANGLEn (6) per set
    b19: MOON_ILLUM_PERCENTn (3) per set
    b18: OTHER_AZIMUTHnj (5), OTHER_ELEVnj (5) per set per other
    b17: SENSOR_AZIMUTHn (5), SENSOR_ELEVn (5) per set
    b16: CATS_ANGLEn (5) per set
    b15: SUN_GLINT_LATn (10), SUN_GLINT_LONn (11) per set
    b14: CATM_ANGLEn (5) per set
    b13: MOON_GLINT_LATn (10), MOON_GLINT_LONn (11) per set
    b12: SUN_ILLUM_METHODnb (1), SUN_ILLUMnb (16) per set per band
    b11: MOON_ILLUM_METHODnb (1), MOON_ILLUMnb (16) per set per band
    b10: SOL_LUN_DIST_ADJUSTn (7), TOT_SUNMOON_ILLUMnb (16) per set per band
    b9:  OTHER_ILLUM_METHODnbj (1), OTHER_ILLUMnbj (16) per set per band per other
    b8:  ART_ILLUM_METHODnb (1), ART_ILLUM_MINnb (16), ART_ILLUM_MAXnb (16) per set per band
    b7-b0: Reserved (always 0)
  
  The header fields, EXISTENCE_MASK, and top-level conditional fields
  (RAD_QUANTITY, RADQ_UNIT) are fully parsed. The illumination set loop
  data is captured as raw bytes because the per-set conditional fields
  reference EXISTENCE_MASK, NUM_BANDS, and NUM_OTHERS from the parent
  scope, which requires _parent resolution not supported by the KSY parser.
  
  Reference: STDI-0002 Volume 1, Appendix AL - ILLUMA-ILLUMB

seq:
  - id: NUM_BANDS
    type: str
    size: 4
    encoding: BCS-N
    doc: "Number of bands. Range: 0001-9999."

  - id: BAND_UNIT
    type: str
    size: 40
    encoding: ECS-A
    doc: "Band unit of measure. Values: um, 1/cm, Hz."

  - id: BANDS
    type: band_bounds
    repeat: expr
    repeat-expr: NUM_BANDS.to_i
    doc: "Band lower and upper bounds, repeated NUM_BANDS times."

  - id: NUM_OTHERS
    type: str
    size: 2
    encoding: BCS-N
    doc: "Number of other natural light sources. Range: 00-99."

  - id: OTHER_NAMES
    type: str
    size: 40
    encoding: ECS-A
    repeat: expr
    repeat-expr: NUM_OTHERS.to_i
    doc: "Name of other natural light source (e.g. VENUS, AURORA)."

  - id: NUM_COMS
    type: str
    size: 1
    encoding: BCS-N
    doc: "Number of ILLUMB comments. Range: 0-9."

  - id: COMMENTS
    type: str
    size: 80
    encoding: ECS-A
    repeat: expr
    repeat-expr: NUM_COMS.to_i
    doc: "Free-form comment. Repeated NUM_COMS times."

  - id: GEO_DATUM
    type: str
    size: 80
    encoding: BCS-A
    doc: "Geodetic datum name. Default: World Geodetic System 1984."

  - id: GEO_DATUM_CODE
    type: str
    size: 4
    encoding: BCS-A
    doc: "Geodetic datum code. Default: WGE (WGS 84)."

  - id: ELLIPSOID_NAME
    type: str
    size: 80
    encoding: BCS-A
    doc: "Ellipsoid name. Default: World Geodetic System 1984."

  - id: ELLIPSOID_CODE
    type: str
    size: 3
    encoding: BCS-A
    doc: "Ellipsoid code. Default: WE (WGS 84)."

  - id: VERTICAL_DATUM_REF
    type: str
    size: 80
    encoding: BCS-A
    doc: "Vertical datum reference. Default: Geodetic."

  - id: VERTICAL_REF_CODE
    type: str
    size: 4
    encoding: BCS-A
    doc: "Vertical reference code. GEOD or MSL."

  - id: EXISTENCE_MASK
    type: u3
    doc: |
      24-bit existence mask controlling conditional fields.
      Bits 23-8 control field presence; bits 7-0 reserved (always 0).

  # --- Bit 23 (0x800000): RAD_QUANTITY, RADQ_UNIT ---
  - id: RAD_QUANTITY
    type: str
    size: 40
    encoding: ECS-A
    if: "EXISTENCE_MASK & 0x800000 != 0"
    doc: "Radiometric quantity for illumination values."

  - id: RADQ_UNIT
    type: str
    size: 40
    encoding: ECS-A
    if: "EXISTENCE_MASK & 0x800000 != 0"
    doc: "Radiometric quantity unit of measure."

  - id: NUM_ILLUM_SETS
    type: str
    size: 3
    encoding: BCS-N
    doc: "Number of illumination condition sets. Range: 001-999."

  # Illumination set loop data captured as raw bytes.
  # Each set contains required fields (DATETIME, TARGET_LAT/LON/HGT)
  # plus conditional fields controlled by EXISTENCE_MASK bits 22-8,
  # with nested per-band and per-other-source loops.
  # Full parsing requires _parent references not supported by KSY parser.
  - id: ILLUM_SET_DATA
    size-eos: true
    doc: |
      Illumination set loop data (NUM_ILLUM_SETS iterations).
      Each set contains:
        Required: DATETIMEn (14), TARGET_LATn (10), TARGET_LONn (11), TARGET_HGTn (14)
        b22: SUN_AZIMUTHn (5), SUN_ELEVn (5)
        b21: MOON_AZIMUTHn (5), MOON_ELEVn (5)
        b20: MOON_PHASE_ANGLEn (6)
        b19: MOON_ILLUM_PERCENTn (3)
        b18: OTHER_AZIMUTHnj (5), OTHER_ELEVnj (5) x NUM_OTHERS
        b17: SENSOR_AZIMUTHn (5), SENSOR_ELEVn (5)
        b16: CATS_ANGLEn (5)
        b15: SUN_GLINT_LATn (10), SUN_GLINT_LONn (11)
        b14: CATM_ANGLEn (5)
        b13: MOON_GLINT_LATn (10), MOON_GLINT_LONn (11)
        b10: SOL_LUN_DIST_ADJUSTn (7)
        Per-band loop (NUM_BANDS iterations):
          b12: SUN_ILLUM_METHODnb (1), SUN_ILLUMnb (16)
          b11: MOON_ILLUM_METHODnb (1), MOON_ILLUMnb (16)
          b10: TOT_SUNMOON_ILLUMnb (16)
          b9: OTHER_ILLUM_METHODnbj (1), OTHER_ILLUMnbj (16) x NUM_OTHERS
          b8: ART_ILLUM_METHODnb (1), ART_ILLUM_MINnb (16), ART_ILLUM_MAXnb (16)

types:
  band_bounds:
    seq:
      - id: LBOUND
        type: str
        size: 16
        encoding: BCS-A
        doc: "Band lower bound in scientific notation."

      - id: UBOUND
        type: str
        size: 16
        encoding: BCS-A
        doc: "Band upper bound in scientific notation."
