meta:
  id: des_csatta
  title: CSATTA DES User-Defined Subheader
  endian: be

doc: |
  CSATTA DES - Coordinate System Attitude Data Extension Segment
  
  Provides sensor attitude information needed to use a rigorous mathematical
  model to perform geolocation and mensuration. This DES provides global
  information for the entire NITF dataset. The CSATTA DES can be repeated
  as necessary if the number of attitude reference points exceeds 9999.
  
  The DESSHL for CSATTA is always 0052 bytes.
  
  Note: This definition covers the DES-specific subheader fields (DESSHF)
  that appear when DESID is "CSATTA". The DESDATA field contains repeated
  quaternion attitude data (4 x 8-byte IEEE 64-bit floats per reference point).
  
  Reference: STDI-0002 Volume 2, Appendix C - CSATTA
  Reference: STDI-0006 (authoritative version)

seq:
  - id: ATT_TYPE
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Type of Attitude Data (ATT_TYPE)
      Type of attitude data being provided.
      12 BCS-A characters.
      Values:
      - ORIGINAL: Original attitude data from the sensor
      - REFINED: Smoothed attitude data based upon provider processing
      Additional values must be registered.

  - id: DT_ATT
    type: str
    size: 14
    encoding: BCS-N
    doc: |
      Time Interval Between Attitude Reference Points (DT_ATT)
      Time interval between attitude reference points in seconds.
      14 BCS-N characters.
      Range: 000.0000000001 to 999.9999999999

  - id: DATE_ATT
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Day of First Attitude Reference Point (DATE_ATT)
      Day at first attitude reference point (UTC).
      8 BCS-N characters in format YYYYMMDD.
      YYYY = Year (2000-9999), MM = Month (01-12), DD = Day (01-31)

  - id: T0_ATT
    type: str
    size: 13
    encoding: BCS-N
    doc: |
      UTC of First Attitude Reference Point (T0_ATT)
      Time of first attitude reference point (UTC).
      13 BCS-N characters in format HHMMSS.mmmmmm.
      HH = Hours (00-23), MM = Minutes (00-59), SS = Seconds (00-59),
      mmmmmm = Microseconds (000000-999999)

  - id: NUM_ATT
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Number of Attitude Reference Points (NUM_ATT)
      Number of attitude reference points throughout the scan interval.
      5 BCS-N characters.
      Range: 00000 to 09999

