meta:
  id: tre_illumb
  title: Illumination TRE (Traditional encoding)
  endian: be

doc: |
  ILLUMB TRE - Illumination Tagged Record Extension (Traditional encoding)
  
  Provides illumination metadata using traditional fixed-field encoding.
  Contains information about natural and artificial illumination relevant
  at the time and location of data collection for electro-optical imagery.
  
  ILLUMB extends ILLUMA with:
  - Multiple illumination condition sets (varying by time, location, wavelength)
  - Target location (latitude, longitude, height)
  - Sensor azimuth and elevation angles
  - Sun and Moon glint locations
  - Camera-to-target-to-Sun (CATS) and Camera-to-target-to-Moon (CATM) angles
  - Other natural light sources (e.g., Venus, Aurora)
  - Per-band illumination values
  
  Uses an EXISTENCE_MASK field to control which conditional fields are present.
  
  NOTE: This is a simplified definition that captures the fixed header fields.
  The full TRE has complex conditional logic based on existence_mask bits
  that requires runtime bitwise evaluation. Conditional fields are captured
  as raw bytes in the conditional_data field.
  
  Reference: STDI-0002 Volume 1, Appendix AL - ILLUMA-ILLUMB

seq:
  - id: num_bands
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Number of Bands (NUM_BANDS)
      Number of bands for which illumination conditions are provided.
      4 BCS-N characters, range 0001-9999.

  - id: band_unit
    type: str
    size: 40
    encoding: ECS-A
    doc: |
      Band Unit of Measure (BAND_UNIT)
      Unit of measure for band lower/upper bounds.
      Values: "μm" (wavelength), "1/cm" (wavenumber), "Hz" (frequency).
      40 ECS-A characters.

  - id: bands
    type: band_bounds
    repeat: expr
    repeat-expr: num_bands.to_i
    doc: |
      Band lower and upper bounds.
      Repeated NUM_BANDS times.

  - id: num_others
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Number of Other Natural Light Sources (NUM_OTHERS)
      Number of natural light sources besides the Sun and Moon.
      2 BCS-N characters, range 00-99.

  - id: other_names
    type: str
    size: 40
    encoding: ECS-A
    repeat: expr
    repeat-expr: num_others.to_i
    doc: |
      Name of Other Natural Light Source (OTHER_NAME)
      Values: "VENUS", "AURORA", etc.
      40 ECS-A characters per source.
      Repeated NUM_OTHERS times.

  - id: num_coms
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Number of ILLUMB Comments (NUM_COMS)
      1 BCS-N character, range 0-9.

  - id: comments
    type: str
    size: 80
    encoding: ECS-A
    repeat: expr
    repeat-expr: num_coms.to_i
    doc: |
      Comment (COMMENT)
      Free-form ECS text. Classified comments preceded by classification.
      80 ECS-A characters per comment.
      Repeated NUM_COMS times.

  - id: geo_datum
    type: str
    size: 80
    encoding: BCS-A
    doc: |
      Geodetic Datum Name (GEO_DATUM)
      Name of geodetic datum for TARGET_LAT and TARGET_LON.
      Default: "World Geodetic System 1984".
      80 BCS-A characters.

  - id: geo_datum_code
    type: str
    size: 4
    encoding: BCS-A
    doc: |
      Geodetic Datum Code (GEO_DATUM_CODE)
      Code of geodetic datum. Default: "WGE" (WGS 84).
      4 BCS-A characters.

  - id: ellipsoid_name
    type: str
    size: 80
    encoding: BCS-A
    doc: |
      Ellipsoid Name (ELLIPSOID_NAME)
      Name of ellipsoid for TARGET fields.
      Default: "World Geodetic System 1984".
      80 BCS-A characters.

  - id: ellipsoid_code
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Ellipsoid Code (ELLIPSOID_CODE)
      Code of ellipsoid. Default: "WE" (WGS 84).
      3 BCS-A characters.

  - id: vertical_datum_ref
    type: str
    size: 80
    encoding: BCS-A
    doc: |
      Vertical Datum Reference (VERTICAL_DATUM_REF)
      Name of vertical datum for TARGET_HGT.
      Default: "Geodetic". BCS spaces if TARGET_HGT not populated.
      80 BCS-A characters.

  - id: vertical_ref_code
    type: str
    size: 4
    encoding: BCS-A
    doc: |
      Vertical Reference Code (VERTICAL_REF_CODE)
      Code of vertical reference. "GEOD" (geodetic) or "MSL" (mean sea level).
      BCS spaces if TARGET_HGT not populated.
      4 BCS-A characters.

  - id: existence_mask
    size: 3
    doc: |
      Existence Mask (EXISTENCE_MASK)
      24-bit field controlling presence of conditional fields.
      Bit 23: RAD_QUANTITY, RADQ_UNIT
      Bit 22: SUN_AZIMUTH, SUN_ELEV
      Bit 21: MOON_AZIMUTH, MOON_ELEV
      Bit 20: MOON_PHASE_ANGLE
      Bit 19: MOON_ILLUM_PERCENT
      Bit 18: OTHER_AZIMUTH, OTHER_ELEV
      Bit 17: SENSOR_AZIMUTH, SENSOR_ELEV
      Bit 16: CATS_ANGLE
      Bit 15: SUN_GLINT_LAT, SUN_GLINT_LON
      Bit 14: CATM_ANGLE
      Bit 13: MOON_GLINT_LAT, MOON_GLINT_LON
      Bit 12: SUN_ILLUM_METHOD, SUN_ILLUM
      Bit 11: MOON_ILLUM_METHOD, MOON_ILLUM
      Bit 10: SOL_LUN_DIST_ADJUST, TOT_SUNMOON_ILLUM
      Bit 9: OTHER_ILLUM_METHOD, OTHER_ILLUM
      Bit 8: ART_ILLUM_METHOD, ART_ILLUM_MIN, ART_ILLUM_MAX
      Bits 0-7: Reserved (always 0)
      3 bytes (unsigned integer).

  - id: num_illum_sets
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Number of Sets of Illumination Conditions (NUM_ILLUM_SETS)
      3 BCS-N characters, range 001-999.

  # Remaining data depends on existence_mask bits
  # This simplified definition captures the raw remaining bytes
  - id: conditional_data
    size-eos: true
    doc: |
      Conditional fields based on existence_mask bits.
      Includes illumination sets with per-band parameters.
      Full parsing requires runtime bitwise evaluation.

types:
  band_bounds:
    seq:
      - id: lbound
        type: str
        size: 16
        encoding: BCS-A
        doc: |
          Band Lower Bound (LBOUND)
          Lower bound of electromagnetic spectrum for this band.
          16 BCS-A characters in scientific notation.

      - id: ubound
        type: str
        size: 16
        encoding: BCS-A
        doc: |
          Band Upper Bound (UBOUND)
          Upper bound of electromagnetic spectrum for this band.
          16 BCS-A characters in scientific notation.
