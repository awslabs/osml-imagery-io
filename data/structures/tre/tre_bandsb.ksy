meta:
  id: tre_bandsb
  title: General Purpose Band Parameters TRE
  endian: be

doc: |
  BANDSB TRE - General Purpose Band Parameters Tagged Record Extension
  
  Provides spectroradiometric metadata for multispectral and hyperspectral
  imagery. Contains cube-level parameters (radiometric quantity, scale factors,
  GSD) and per-band parameters (wavelength, calibration, noise, etc.).
  
  The EXISTENCE_MASK field (32-bit unsigned integer) controls which conditional
  fields are present. Each bit in the mask corresponds to specific optional
  field groups. Per-band fields repeat COUNT times.
  
  Bit definitions (bit 31 = MSB, bit 0 = LSB):
    b31: RADIOMETRIC_ADJUSTMENT_SURFACE, ATMOSPHERIC_ADJUSTMENT_ALTITUDE
    b30: DIAMETER
    b29: DATA_FLD_2
    b28: BANDIDn (per-band)
    b27: BAD_BANDn (per-band)
    b26: NIIRSn (per-band)
    b25: FOCAL_LENn (per-band)
    b24: CWAVEn (per-band) — also requires WAVE_LENGTH_UNIT
    b23: FWHMn (per-band)
    b22: FWHM_UNCn (per-band) — if set, b23 must also be set
    b21: NOM_WAVEn (per-band)
    b20: NOM_WAVE_UNCn (per-band) — if set, b21 must also be set
    b19: LBOUNDn, UBOUNDn (per-band)
    b18: SCALE_FACTORn, ADDITIVE_FACTORn (per-band, IEEE754)
    b17: START_TIMEn (per-band)
    b16: INT_TIMEn (per-band)
    b15: CALDRKn, CALIBRATION_SENSITIVITYn (per-band)
    b14: ROW_GSDn, ROW_GSD_UNITn, COL_GSDn, COL_GSD_UNITn (per-band)
    b13: ROW_GSD_UNCn, ROW_GSD_UNITn, COL_GSD_UNCn, COL_GSD_UNITn (per-band) — if set, b14 must also be set
    b12: BKNOISEn, SCNNOISEn (per-band)
    b11: SPT_RESP_FUNCTION_ROWn, SPT_RESP_UNIT_ROWn, SPT_RESP_FUNCTION_COLn, SPT_RESP_UNIT_COLn (per-band)
    b10: SPT_RESP_UNC_ROWn, SPT_RESP_UNIT_ROWn, SPT_RESP_UNC_COLn, SPT_RESP_UNIT_COLn (per-band) — if set, b11 must also be set
    b9:  DATA_FLD_3n (per-band)
    b8:  DATA_FLD_4n (per-band)
    b7:  DATA_FLD_5n (per-band)
    b6:  DATA_FLD_6n (per-band)
    b5-b1: Reserved (always 0)
    b0:  NUM_AUX_B, NUM_AUX_C, auxiliary parameter loops
  
  Note: The auxiliary parameter section (b0) contains switch-on-value logic
  (BAPFm selects between numeric, IEEE754, or ASCII formats) which is not
  fully supported by the KSY parser. Auxiliary data is captured as raw bytes.
  
  Variable length TRE (minimum 122 bytes)
  
  Reference: STDI-0002 Volume 1, Appendix X - BANDSB

seq:
  - id: COUNT
    type: str
    size: 5
    encoding: BCS-N
    doc: "Number of bands comprising the spectral cube. Range: 00001-99999"

  - id: RADIOMETRIC_QUANTITY
    type: str
    size: 24
    encoding: BCS-A
    doc: |
      Data representation. Values include:
      EMISSIVITY, REFLECTANCE, EMITTANCE, IRRADIANCE,
      KINETIC TEMPERATURE, RADIANT TEMPERATURE, RADIANCE,
      RADIANT FLUX, THERMAL INERTIA, APPARENT THERMAL INERTIA,
      UNCALIBRATED, RAW

  - id: RADIOMETRIC_QUANTITY_UNIT
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Unit of measure for radiometric data.
      P = Percentage, E = W/m^2, K = kelvin, L = W/(m^2 sr),
      F = Watts, X = W/(m^2 um), S = W/(m^2 sr um),
      U = microflicks, I = J/(m^2 s^1/2 K), A = K^-1,
      D = Digital Number, V = Volts, N = None

  - id: SCALE_FACTOR
    size: 4
    doc: "Cube scale factor (M). IEEE 754 single-precision float."

  - id: ADDITIVE_FACTOR
    size: 4
    doc: "Cube additive factor (A). IEEE 754 single-precision float."

  - id: ROW_GSD
    type: str
    size: 7
    encoding: BCS-N
    doc: "Row ground sample distance. Range: 000.001 to 9999.99"

  - id: ROW_GSD_UNIT
    type: str
    size: 1
    encoding: BCS-A
    doc: "Unit of row GSD. M = meters, R = microradians"

  - id: COL_GSD
    type: str
    size: 7
    encoding: BCS-N
    doc: "Column ground sample distance. Range: 000.001 to 9999.99"

  - id: COL_GSD_UNIT
    type: str
    size: 1
    encoding: BCS-A
    doc: "Unit of column GSD. M = meters, R = microradians"

  - id: SPT_RESP_ROW
    type: str
    size: 7
    encoding: BCS-N
    doc: "Spatial response function across rows."

  - id: SPT_RESP_UNIT_ROW
    type: str
    size: 1
    encoding: BCS-A
    doc: "Unit of row spatial response. M = meters, R = microradians"

  - id: SPT_RESP_COL
    type: str
    size: 7
    encoding: BCS-N
    doc: "Spatial response function across columns."

  - id: SPT_RESP_UNIT_COL
    type: str
    size: 1
    encoding: BCS-A
    doc: "Unit of column spatial response. M = meters, R = microradians"

  - id: DATA_FLD_1
    size: 48
    doc: "Reserved for future use."

  - id: EXISTENCE_MASK
    type: u4
    doc: "32-bit existence mask controlling conditional fields."

  - id: WAVE_LENGTH_UNIT
    type: str
    size: 1
    encoding: BCS-A
    doc: "Wavelength unit. U = micrometers, W = wavenumber (cm^-1)"

  # --- Bit 31 (0x80000000): Radiometric adjustment surface and atmospheric altitude ---
  - id: RADIOMETRIC_ADJUSTMENT_SURFACE
    type: str
    size: 24
    encoding: BCS-A
    if: "EXISTENCE_MASK & 0x80000000 != 0"
    doc: "Radiometric adjustment surface description."

  - id: ATMOSPHERIC_ADJUSTMENT_ALTITUDE
    size: 4
    if: "EXISTENCE_MASK & 0x80000000 != 0"
    doc: "Atmospheric adjustment altitude. IEEE 754 single-precision float."

  # --- Bit 30 (0x40000000): Diameter ---
  - id: DIAMETER
    type: str
    size: 7
    encoding: BCS-N
    if: "EXISTENCE_MASK & 0x40000000 != 0"
    doc: "Diameter of the collecting aperture. Range: 000.001 to 9999.99"

  # --- Bit 29 (0x20000000): DATA_FLD_2 ---
  - id: DATA_FLD_2
    size: 32
    if: "EXISTENCE_MASK & 0x20000000 != 0"
    doc: "Reserved for future use."

  # --- Bit 28 (0x10000000): BANDIDn (per-band) ---
  - id: BANDID
    type: str
    size: 50
    encoding: BCS-A
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x10000000 != 0"
    doc: "Band identifier string. 50 BCS-A per band."

  # --- Bit 27 (0x08000000): BAD_BANDn (per-band) ---
  - id: BAD_BAND
    type: str
    size: 1
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x08000000 != 0"
    doc: "Bad band indicator. 0 = good, 1 = bad."

  # --- Bit 26 (0x04000000): NIIRSn (per-band) ---
  - id: NIIRS
    type: str
    size: 3
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x04000000 != 0"
    doc: "National Imagery Interpretability Rating Scale value per band."

  # --- Bit 25 (0x02000000): FOCAL_LENn (per-band) ---
  - id: FOCAL_LEN
    type: str
    size: 5
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x02000000 != 0"
    doc: "Focal length per band in millimeters."

  # --- Bit 24 (0x01000000): CWAVEn (per-band) ---
  - id: CWAVE
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x01000000 != 0"
    doc: "Center wavelength per band. Units from WAVE_LENGTH_UNIT."

  # --- Bit 23 (0x00800000): FWHMn (per-band) ---
  - id: FWHM
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00800000 != 0"
    doc: "Full width at half maximum per band."

  # --- Bit 22 (0x00400000): FWHM_UNCn (per-band) ---
  # Note: If b22 is set, b23 must also be set per spec
  - id: FWHM_UNC
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00400000 != 0"
    doc: "FWHM uncertainty per band. Requires b23 (FWHM) also set."

  # --- Bit 21 (0x00200000): NOM_WAVEn (per-band) ---
  - id: NOM_WAVE
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00200000 != 0"
    doc: "Nominal wavelength per band."

  # --- Bit 20 (0x00100000): NOM_WAVE_UNCn (per-band) ---
  # Note: If b20 is set, b21 must also be set per spec
  - id: NOM_WAVE_UNC
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00100000 != 0"
    doc: "Nominal wavelength uncertainty per band. Requires b21 (NOM_WAVE) also set."

  # --- Bit 19 (0x00080000): LBOUNDn, UBOUNDn (per-band) ---
  - id: LBOUND
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00080000 != 0"
    doc: "Lower spectral bound per band."

  - id: UBOUND
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00080000 != 0"
    doc: "Upper spectral bound per band."

  # --- Bit 18 (0x00040000): SCALE_FACTORn, ADDITIVE_FACTORn (per-band, IEEE754) ---
  - id: BAND_SCALE_FACTOR
    size: 4
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00040000 != 0"
    doc: "Per-band scale factor. IEEE 754 single-precision float."

  - id: BAND_ADDITIVE_FACTOR
    size: 4
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00040000 != 0"
    doc: "Per-band additive factor. IEEE 754 single-precision float."

  # --- Bit 17 (0x00020000): START_TIMEn (per-band) ---
  - id: START_TIME
    type: str
    size: 16
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00020000 != 0"
    doc: "Start time per band. YYYYMMDDhhmmss.s format."

  # --- Bit 16 (0x00010000): INT_TIMEn (per-band) ---
  - id: INT_TIME
    type: str
    size: 6
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00010000 != 0"
    doc: "Integration time per band in milliseconds."

  # --- Bit 15 (0x00008000): CALDRKn, CALIBRATION_SENSITIVITYn (per-band) ---
  - id: CALDRK
    type: str
    size: 6
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00008000 != 0"
    doc: "Calibration dark current per band."

  - id: CALIBRATION_SENSITIVITY
    type: str
    size: 5
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00008000 != 0"
    doc: "Calibration sensitivity per band."

  # --- Bit 14 (0x00004000): ROW_GSDn, ROW_GSD_UNITn, COL_GSDn, COL_GSD_UNITn (per-band) ---
  - id: BAND_ROW_GSD
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00004000 != 0"
    doc: "Per-band row ground sample distance."

  - id: BAND_ROW_GSD_UNIT
    type: str
    size: 1
    encoding: BCS-A
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00004000 != 0"
    doc: "Per-band row GSD unit. M = meters, R = microradians."

  - id: BAND_COL_GSD
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00004000 != 0"
    doc: "Per-band column ground sample distance."

  - id: BAND_COL_GSD_UNIT
    type: str
    size: 1
    encoding: BCS-A
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00004000 != 0"
    doc: "Per-band column GSD unit. M = meters, R = microradians."

  # --- Bit 13 (0x00002000): ROW_GSD_UNCn, ROW_GSD_UNITn, COL_GSD_UNCn, COL_GSD_UNITn (per-band) ---
  # Note: If b13 is set, b14 must also be set per spec
  - id: BAND_ROW_GSD_UNC
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00002000 != 0"
    doc: "Per-band row GSD uncertainty. Requires b14 also set."

  - id: BAND_ROW_GSD_UNC_UNIT
    type: str
    size: 1
    encoding: BCS-A
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00002000 != 0"
    doc: "Per-band row GSD uncertainty unit."

  - id: BAND_COL_GSD_UNC
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00002000 != 0"
    doc: "Per-band column GSD uncertainty. Requires b14 also set."

  - id: BAND_COL_GSD_UNC_UNIT
    type: str
    size: 1
    encoding: BCS-A
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00002000 != 0"
    doc: "Per-band column GSD uncertainty unit."

  # --- Bit 12 (0x00001000): BKNOISEn, SCNNOISEn (per-band) ---
  - id: BKNOISE
    type: str
    size: 5
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00001000 != 0"
    doc: "Background noise level per band."

  - id: SCNNOISE
    type: str
    size: 5
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00001000 != 0"
    doc: "Scanner noise level per band."

  # --- Bit 11 (0x00000800): SPT_RESP_FUNCTION_ROWn, SPT_RESP_UNIT_ROWn, SPT_RESP_FUNCTION_COLn, SPT_RESP_UNIT_COLn (per-band) ---
  - id: BAND_SPT_RESP_FUNCTION_ROW
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00000800 != 0"
    doc: "Per-band spatial response function across rows."

  - id: BAND_SPT_RESP_UNIT_ROW
    type: str
    size: 1
    encoding: BCS-A
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00000800 != 0"
    doc: "Per-band row spatial response unit."

  - id: BAND_SPT_RESP_FUNCTION_COL
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00000800 != 0"
    doc: "Per-band spatial response function across columns."

  - id: BAND_SPT_RESP_UNIT_COL
    type: str
    size: 1
    encoding: BCS-A
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00000800 != 0"
    doc: "Per-band column spatial response unit."

  # --- Bit 10 (0x00000400): SPT_RESP_UNC_ROWn, SPT_RESP_UNIT_ROWn, SPT_RESP_UNC_COLn, SPT_RESP_UNIT_COLn (per-band) ---
  # Note: If b10 is set, b11 must also be set per spec
  - id: BAND_SPT_RESP_UNC_ROW
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00000400 != 0"
    doc: "Per-band spatial response uncertainty across rows. Requires b11 also set."

  - id: BAND_SPT_RESP_UNC_UNIT_ROW
    type: str
    size: 1
    encoding: BCS-A
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00000400 != 0"
    doc: "Per-band row spatial response uncertainty unit."

  - id: BAND_SPT_RESP_UNC_COL
    type: str
    size: 7
    encoding: BCS-N
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00000400 != 0"
    doc: "Per-band spatial response uncertainty across columns. Requires b11 also set."

  - id: BAND_SPT_RESP_UNC_UNIT_COL
    type: str
    size: 1
    encoding: BCS-A
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00000400 != 0"
    doc: "Per-band column spatial response uncertainty unit."

  # --- Bit 9 (0x00000200): DATA_FLD_3n (per-band) ---
  - id: DATA_FLD_3
    size: 16
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00000200 != 0"
    doc: "Reserved per-band data field 3."

  # --- Bit 8 (0x00000100): DATA_FLD_4n (per-band) ---
  - id: DATA_FLD_4
    size: 24
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00000100 != 0"
    doc: "Reserved per-band data field 4."

  # --- Bit 7 (0x00000080): DATA_FLD_5n (per-band) ---
  - id: DATA_FLD_5
    size: 32
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00000080 != 0"
    doc: "Reserved per-band data field 5."

  # --- Bit 6 (0x00000040): DATA_FLD_6n (per-band) ---
  - id: DATA_FLD_6
    size: 48
    repeat: expr
    repeat-expr: COUNT.to_i
    if: "EXISTENCE_MASK & 0x00000040 != 0"
    doc: "Reserved per-band data field 6."

  # --- Bits 5-1: Reserved (always 0, no fields) ---

  # --- Bit 0 (0x00000001): Auxiliary parameters ---
  # The auxiliary parameter section has switch-on-value logic (BAPFm/CAPFk
  # selects between numeric, IEEE754, or ASCII formats) which is not supported
  # by the KSY parser. Captured as raw bytes when present.
  - id: AUXILIARY_DATA
    size-eos: true
    if: "EXISTENCE_MASK & 0x00000001 != 0"
    doc: |
      Auxiliary parameter data (bit 0 set).
      Contains NUM_AUX_B (2 BCS-N), NUM_AUX_C (2 BCS-N), then:
        - For each of NUM_AUX_B auxiliary B parameters:
          BAPFm (1 BCS-A): format selector (I=integer, R=real, A=ASCII)
          UBANDm (7 BCS-A): unit of measure
          Then per-band: APNmn (10 BCS-N) if I, APRmn (4 IEEE754) if R, APAmn (20 BCS-A) if A
        - For each of NUM_AUX_C auxiliary C parameters:
          CAPFk (1 BCS-A): format selector
          UCBANDk (7 BCS-A): unit of measure
          Then per-band: APNkn/APRkn/APAkn based on CAPFk
      Captured as raw bytes due to switch-on-value limitation.
