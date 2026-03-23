meta:
  id: tre_csepha
  title: Ephemeris Data TRE
  endian: be

doc: |
  CSEPHA TRE - Ephemeris Data
  
  Provides detailed space vehicle ephemeris information. The CSEPHA TRE
  provides global information for the entire NITF dataset.
  
  The CSEPHA can be repeated as necessary if the number of ephemeris vectors
  exceeds 999 in order to contain all ephemeris data. When multiple CSEPHA
  TREs are required, the remaining vectors are recorded across multiple
  instances in time-sequence order.
  
  Minimum number of ephemeris vectors is 7: 3 during the pre-imaging interval
  and 3 during the post-imaging interval.
  
  This TRE resides in the TRE_OVERFLOW DES for each sensor.
  
  Reference: STDI-0006 (NCDRD), Table 3.4-1

seq:
  - id: EPHEM_FLAG
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Ephemeris Flag
      Source of orbit determination ephemeris data.
      PREDICTED = predicted ephemeris
      COLLECT-TIME = actual real time ephemeris
      REFINED = refined real time ephemeris

  - id: DT_EPHEM
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Time interval between ephemeris vectors in seconds.
      Range: 000.1 to 999.9
      Note: A positive value is always provided even when
      TIME_IMAGE_DURATION is negative.

  - id: DATE_EPHEM
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Day of First Ephemeris Vector (UTC)
      Format: YYYYMMDD

  - id: T0_EPHEM
    type: str
    size: 13
    encoding: BCS-N
    doc: |
      UTC of First Ephemeris Vector
      Format: HHMMSS.mmmmmm (hours, minutes, seconds, microseconds)
      Range: 00-23, 00-59, 00.000000-59.999999

  - id: NUM_EPHEM
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Number of Ephemeris Vectors
      Range: 001 to 999

  - id: EPHEM_DATA
    type: ephemeris_vector
    repeat: expr
    repeat-expr: NUM_EPHEM.to_i
    doc: Ephemeris vectors in ECEF coordinates.

types:
  ephemeris_vector:
    doc: |
      A single ephemeris vector containing X, Y, Z coordinates
      in Earth Centered Earth Fixed (ECEF) coordinate system.
    seq:
      - id: EPHEM_X
        type: str
        size: 12
        encoding: BCS-N
        doc: |
          X Coordinate of Ephemeris Vector in ECEF coordinates.
          Range: -99999999.99 to +99999999.99 meters.

      - id: EPHEM_Y
        type: str
        size: 12
        encoding: BCS-N
        doc: |
          Y Coordinate of Ephemeris Vector in ECEF coordinates.
          Range: -99999999.99 to +99999999.99 meters.

      - id: EPHEM_Z
        type: str
        size: 12
        encoding: BCS-N
        doc: |
          Z Coordinate of Ephemeris Vector in ECEF coordinates.
          Range: -99999999.99 to +99999999.99 meters.
