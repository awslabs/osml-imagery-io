meta:
  id: tre_bandsb
  title: General Purpose Band Parameters TRE
  endian: be

doc: |
  BANDSB TRE - General Purpose Band Parameters Tagged Record Extension
  
  Provides spectroradiometric metadata for multispectral and hyperspectral
  imagery. Contains cube-level parameters (radiometric quantity, scale factors,
  GSD) and per-band parameters (wavelength, calibration, noise, etc.).
  
  The EXISTENCE_MASK field controls which conditional fields are present.
  Each bit in the mask corresponds to specific optional fields.
  
  NOTE: This is a simplified definition that captures the fixed header fields.
  The full TRE has complex conditional logic based on the existence_mask bits
  that requires runtime bitwise evaluation. Per-band conditional fields are
  not fully implemented in this definition.
  
  Variable length TRE (minimum 122 bytes)
  
  Reference: STDI-0002 Volume 1, Appendix X - BANDSB

seq:
  - id: count
    type: str
    size: 5
    encoding: BCS-N
    doc: "Number of bands comprising the spectral cube. Range: 00001-99999"

  - id: radiometric_quantity
    type: str
    size: 24
    encoding: BCS-A
    doc: |
      Data representation. Values include:
      EMISSIVITY, REFLECTANCE, EMITTANCE, IRRADIANCE,
      KINETIC TEMPERATURE, RADIANT TEMPERATURE, RADIANCE,
      RADIANT FLUX, THERMAL INERTIA, APPARENT THERMAL INERTIA,
      UNCALIBRATED, RAW

  - id: radiometric_quantity_unit
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Unit of measure for radiometric data.
      P = Percentage, E = W/m^2, K = kelvin, L = W/(m^2 sr),
      F = Watts, X = W/(m^2 um), S = W/(m^2 sr um),
      U = microflicks, I = J/(m^2 s^1/2 K), A = K^-1,
      D = Digital Number, V = Volts, N = None

  - id: scale_factor
    size: 4
    doc: |
      Cube scale factor (M). IEEE 754 single-precision float.
      Multiplicative factor applied to all bands.

  - id: additive_factor
    size: 4
    doc: |
      Cube additive factor (A). IEEE 754 single-precision float.
      Constant added to all bands after scale factor.

  - id: row_gsd
    type: str
    size: 7
    encoding: BCS-N
    doc: "Row ground sample distance. Range: 000.001 to 9999.99"

  - id: row_gsd_unit
    type: str
    size: 1
    encoding: BCS-A
    doc: "Unit of row GSD. M = meters, R = microradians"

  - id: col_gsd
    type: str
    size: 7
    encoding: BCS-N
    doc: "Column ground sample distance. Range: 000.001 to 9999.99"

  - id: col_gsd_unit
    type: str
    size: 1
    encoding: BCS-A
    doc: "Unit of column GSD. M = meters, R = microradians"

  - id: spt_resp_row
    type: str
    size: 7
    encoding: BCS-N
    doc: Spatial response function across rows.

  - id: spt_resp_unit_row
    type: str
    size: 1
    encoding: BCS-A
    doc: "Unit of row spatial response. M = meters, R = microradians"

  - id: spt_resp_col
    type: str
    size: 7
    encoding: BCS-N
    doc: Spatial response function across columns.

  - id: spt_resp_unit_col
    type: str
    size: 1
    encoding: BCS-A
    doc: "Unit of column spatial response. M = meters, R = microradians"

  - id: data_fld_1
    size: 48
    doc: Reserved for future use.

  - id: existence_mask
    type: u4
    doc: |
      32-bit existence mask controlling conditional fields.
      Each bit enables specific optional fields in the TRE.
      See STDI-0002 Appendix X for bit definitions.

  - id: wave_length_unit
    type: str
    size: 1
    encoding: BCS-A
    doc: "Wavelength unit. U = micrometers, W = wavenumber (cm^-1)"

  # Remaining data depends on existence_mask bits
  # This simplified definition captures the raw remaining bytes
  - id: conditional_data
    size-eos: true
    doc: |
      Conditional fields based on existence_mask bits.
      Includes per-band parameters and auxiliary data.
      Full parsing requires runtime bitwise evaluation.
