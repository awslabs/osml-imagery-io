meta:
  id: tre_bandsa
  title: Multispectral/Hyperspectral Band Parameters TRE (Legacy)
  endian: be

doc: |
  BANDSA TRE - Multispectral/Hyperspectral Band Parameters Tagged Record Extension
  
  LEGACY TRE - Inactive since 1 August 2007. Superseded by BANDSB.
  
  This TRE was designed to supplement information in the NITF image segment
  subheader where additional parametric data are required for multispectral
  and hyperspectral imagery. It provides band-level metadata including
  wavelength information, calibration data, and noise characteristics.
  
  The BANDSA TRE was part of the Airborne Support Data Extensions (ASDE)
  and was considered an airborne TRE. Unlike BANDSB which is platform
  independent, BANDSA was primarily intended for airborne sensor platforms.
  
  Variable length TRE (72 to 45980 bytes)
  
  Reference: STDI-0002 Volume 1, Appendix E - ASDE (Version 3.0, 2007)
  Note: Current STDI-0002 documents mark this TRE as inactive.
  
  For new implementations, use BANDSB instead.

seq:
  - id: ROW_SPACING
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Row spacing in meters.
      Range: 0000.01 to 9999.99 meters.
      "-------" if unknown.

  - id: ROW_SPACING_UNITS
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Row spacing units.
      M = meters, R = microradians.

  - id: COL_SPACING
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Column spacing in meters.
      Range: 0000.01 to 9999.99 meters.
      "-------" if unknown.

  - id: COL_SPACING_UNITS
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Column spacing units.
      M = meters, R = microradians.

  - id: FOCAL_LENGTH
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Focal length in millimeters.
      Range: 0001.0 to 9999.9 mm.
      "------" if unknown.

  - id: COUNT
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Number of bands in the spectral cube.
      Range: 0001 to 9999.

  # Per-band fields loop
  - id: BANDS
    type: band_entry
    repeat: expr
    repeat-expr: COUNT.to_i
    doc: Per-band parameters.

types:
  band_entry:
    doc: |
      Per-band parameters for BANDSA TRE.
      Each band entry contains wavelength and calibration information.
    seq:
      - id: BANDID
        type: str
        size: 6
        encoding: BCS-A
        doc: |
          Band identifier.
          6 BCS-A characters.

      - id: BAD_BAND
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Bad band indicator.
          0 = bad/invalid band, 1 = good/valid band.

      - id: START_WAVE
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          Start wavelength in micrometers.
          Range: 00.0001 to 99.9999 micrometers.
          "-------" if unknown.

      - id: CENTER_WAVE
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          Center wavelength in micrometers.
          Range: 00.0001 to 99.9999 micrometers.
          "-------" if unknown.

      - id: END_WAVE
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          End wavelength in micrometers.
          Range: 00.0001 to 99.9999 micrometers.
          "-------" if unknown.

      - id: RADIOMETRIC_CAL
        type: str
        size: 5
        encoding: BCS-N
        doc: |
          Radiometric calibration coefficient.
          Range: 0.001 to 9.999.
          "-----" if unknown.

      - id: CAL_DARK
        type: str
        size: 6
        encoding: BCS-N
        doc: |
          Calibration dark value.
          Range: 000000 to 999999.
          "------" if unknown.

      - id: CAL_SENSITIVITY
        type: str
        size: 5
        encoding: BCS-N
        doc: |
          Calibration sensitivity.
          Range: 00.01 to 99.99.
          "-----" if unknown.

      - id: NOISE_LEVEL
        type: str
        size: 5
        encoding: BCS-N
        doc: |
          Noise level (noise equivalent radiance or reflectance).
          Range: 0.001 to 9.999.
          "-----" if unknown.

