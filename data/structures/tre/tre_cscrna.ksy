meta:
  id: tre_cscrna
  title: Corner Footprint TRE
  endian: be

doc: |
  CSCRNA TRE - Corner Footprint
  
  Provides the geodetic latitude, longitude, and ground elevation at the
  four corners of the sensor (sub-image) footprint (or MBR, if the footprint
  is of irregular shape).
  
  If the data for a given sensor (sub-image) spans multiple image segments,
  the CSCRNA TRE shall be identical in each of the image segments and shall
  represent the extent of the entire sub-image.
  
  This is a required TRE that resides in image segment subheaders.
  
  Reference: STDI-0006 (NCDRD), Table 3.2-1

seq:
  - id: PREDICT_CORNERS
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Predicted Corners Flag
      Indicator of whether the corner coordinates are predicted or
      based on actual measurements.
      Y = Predicted, N = Actual

  - id: ULCNR_LAT
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Upper Left Corner Latitude
      Geodetic latitude in decimal degrees (+dd.ddddd).
      Range: -90.00000 to +90.00000
      '+' = northern hemisphere, '-' = southern hemisphere

  - id: ULCNR_LONG
    type: str
    size: 10
    encoding: BCS-N
    doc: |
      Upper Left Corner Longitude
      Geodetic longitude in decimal degrees (+ddd.ddddd).
      Range: -179.99999 to +180.00000
      '+' = eastern hemisphere, '-' = western hemisphere

  - id: ULCNR_HT
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Upper Left Corner Height
      Height referenced to WGS-84 ellipsoid in meters.
      Range: -00610.0 to +10668.0

  - id: URCNR_LAT
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Upper Right Corner Latitude
      Geodetic latitude in decimal degrees (+dd.ddddd).
      Range: -90.00000 to +90.00000

  - id: URCNR_LONG
    type: str
    size: 10
    encoding: BCS-N
    doc: |
      Upper Right Corner Longitude
      Geodetic longitude in decimal degrees (+ddd.ddddd).
      Range: -179.99999 to +180.00000

  - id: URCNR_HT
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Upper Right Corner Height
      Height referenced to WGS-84 ellipsoid in meters.
      Range: -00610.0 to +10668.0

  - id: LRCNR_LAT
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Lower Right Corner Latitude
      Geodetic latitude in decimal degrees (+dd.ddddd).
      Range: -90.00000 to +90.00000

  - id: LRCNR_LONG
    type: str
    size: 10
    encoding: BCS-N
    doc: |
      Lower Right Corner Longitude
      Geodetic longitude in decimal degrees (+ddd.ddddd).
      Range: -179.99999 to +180.00000

  - id: LRCNR_HT
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Lower Right Corner Height
      Height referenced to WGS-84 ellipsoid in meters.
      Range: -00610.0 to +10668.0

  - id: LLCNR_LAT
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Lower Left Corner Latitude
      Geodetic latitude in decimal degrees (+dd.ddddd).
      Range: -90.00000 to +90.00000

  - id: LLCNR_LONG
    type: str
    size: 10
    encoding: BCS-N
    doc: |
      Lower Left Corner Longitude
      Geodetic longitude in decimal degrees (+ddd.ddddd).
      Range: -179.99999 to +180.00000

  - id: LLCNR_HT
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Lower Left Corner Height
      Height referenced to WGS-84 ellipsoid in meters.
      Range: -00610.0 to +10668.0
