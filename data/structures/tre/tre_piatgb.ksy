meta:
  id: tre_piatgb
  title: Profile for Imagery Access Target TRE
  endian: be

doc: |
  PIATGB TRE - Profile for Imagery Access Target Support Extension - Version B
  
  Contains descriptive data about targets identified in imagery.
  Present once for each target identified in the image, up to 250 per data type.
  
  Reference: STDI-0002 Volume 1, Appendix C - PIAE

seq:
  - id: TGTUTM
    type: str
    size: 15
    encoding: ASCII
    doc: |
      Target UTM (TGTUTM)
      Universal Transverse Mercator grid coordinates.
      15 BCS-A, XXXNNnnnnnnnnnn format.

  - id: PIATGAID
    type: str
    size: 15
    encoding: ASCII
    doc: |
      Target Identification (PIATGAID)
      Basic Encyclopedia (BE) or non-BE ID of primary target.
      15 BCS-A.

  - id: PIACTRY
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Country Code (PIACTRY)
      Country where target coordinates reside.
      2 BCS-A, GEC code.

  - id: PIACAT
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Category Code (PIACAT)
      Target classification by product or activity type.
      5 BCS-N, DIAM 65-3-1.

  - id: TGTGEO
    type: str
    size: 15
    encoding: ASCII
    doc: |
      Target Geographic Coordinates (TGTGEO)
      Point target geographic location.
      15 BCS-A, ddmmssXdddmmssY format.

  - id: DATUM
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Target Coordinate Datum (DATUM)
      Datum of map used to derive target coordinates.
      3 BCS-A.

  - id: TGTNAME
    type: str
    size: 38
    encoding: ASCII
    doc: |
      Target Name (TGTNAME)
      Official name of target element based on MIIDS/IDB.
      38 BCS-A.

  - id: PERCOVER
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Percentage of Coverage (PERCOVER)
      Percentage of target covered by image.
      3 BCS-N, 000-100.

  - id: TGTLAT
    type: str
    size: 10
    encoding: ASCII
    doc: |
      Target Latitude (TGTLAT)
      Point target latitude in decimal degrees.
      10 BCS-N, +dd.dddddd format.

  - id: TGTLON
    type: str
    size: 11
    encoding: ASCII
    doc: |
      Target Longitude (TGTLON)
      Point target longitude in decimal degrees.
      11 BCS-N, +ddd.dddddd format.
