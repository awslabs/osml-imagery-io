meta:
  id: tre_cssfaa
  title: Sensor Field Alignment Data TRE
  endian: be

doc: |
  CSSFAA TRE - Sensor Field Alignment Data
  
  Provides information on detectors, sensor type, and field alignment
  including fields for the focal length and principal point offset
  components. This TRE provides global information for the entire NITF
  dataset.
  
  When included in a dataset, the TRE shall provide field alignment data
  for each band (panchromatic, multispectral band 1, etc.) represented
  in the wideband data of the dataset.
  
  This TRE resides in the TRE_OVERFLOW DES for each sensor.
  
  Reference: STDI-0006 (NCDRD), Table 3.7-1

seq:
  - id: NUM_BANDS
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Number of Bands
      Number of bands in segment.
      Range: 1 to number of bands supplied by CDP

  - id: BANDS
    type: band_alignment
    repeat: expr
    repeat-expr: NUM_BANDS.to_i
    doc: Field alignment data for each band.

types:
  band_alignment:
    doc: |
      Field alignment data for a single band including focal length,
      detector array geometry, and principal point offsets.
    seq:
      - id: BAND_TYPE
        type: str
        size: 1
        encoding: BCS-A
        doc: |
          Category of band.
          PAN = M
          MS = R, G, B, N, or space

      - id: BAND_ID
        type: str
        size: 6
        encoding: BCS-A
        doc: |
          Band center wavelength in nanometers.
          Populated with values identical to those used for ISUBCATn
          as specified in NCDRD Table 2.1-3.

      - id: FOC_LENGTH
        type: str
        size: 11
        encoding: BCS-N
        doc: |
          Focal Length in millimeters.
          Range: 00000.00001 to 99999.99999

      - id: NUM_DAP
        type: str
        size: 8
        encoding: BCS-N
        doc: |
          Number of linear arrays (pairs) for a band.
          For a Basic product only.
          Value: 00000001

      - id: NUM_FIR
        type: str
        size: 8
        encoding: BCS-N
        doc: |
          First sample number.
          Value: 00000001

      - id: DELTA
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          Number of detector elements in a linear array.
          Range: 1 to 9999999

      - id: OPPOFF_X
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          Principal point offset X in meters.
          Range: -100.00 to +100.00

      - id: OPPOFF_Y
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          Principal point offset Y in meters.
          Range: -100.00 to +100.00

      - id: OPPOFF_Z
        type: str
        size: 7
        encoding: BCS-N
        doc: |
          Principal point offset Z in meters.
          Range: -100.00 to +100.00

      - id: START_X
        type: str
        size: 11
        encoding: BCS-N
        doc: |
          Detector mounting of the first pixel in the pair - X.
          Range: -99999.9999 to +99999.9999 millimeters.

      - id: START_Y
        type: str
        size: 11
        encoding: BCS-N
        doc: |
          Detector mounting of the first pixel in the pair - Y.
          Range: -99999.9999 to +99999.9999 millimeters.

      - id: FINISH_X
        type: str
        size: 11
        encoding: BCS-N
        doc: |
          Detector mounting of the last pixel in the pair - X.
          Range: -99999.9999 to +99999.9999 millimeters.

      - id: FINISH_Y
        type: str
        size: 11
        encoding: BCS-N
        doc: |
          Detector mounting of the last pixel in the pair - Y.
          Range: -99999.9999 to +99999.9999 millimeters.
