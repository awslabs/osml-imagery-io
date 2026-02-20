meta:
  id: tre_rsmida
  title: RSM Identification TRE
  endian: be

doc: |
  RSMIDA TRE - Replacement Sensor Model Identification
  
  Provides identification and support data for the RSM. Contains image
  identification, sensor identification, timing, ground reference point,
  image domain, illumination, and sensor array information.
  
  CEL: 1628 bytes
  
  Reference: STDI-0002 Volume 1, Appendix U - RSM

seq:
  - id: IID
    type: str
    size: 80
    encoding: BCS-A
    doc: |
      Image Identifier
      80 BCS-A characters identifying the image.

  - id: EDITION
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      RSM Image Support Data Edition
      40 BCS-A characters identifying the edition.

  - id: ISID
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      Image Sequence Identifier
      40 BCS-A characters identifying the image sequence.

  - id: SID
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      Sensor Identifier
      40 BCS-A characters identifying the sensor.

  - id: STID
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      Sensor Type Identifier
      40 BCS-A characters identifying the sensor type.

  - id: YEAR
    type: str
    size: 4
    encoding: BCS-NPI
    doc: |
      Year of Image Acquisition
      4 BCS-NPI digits (YYYY).

  - id: MONTH
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Month of Image Acquisition
      2 BCS-NPI digits (MM), range 01-12.

  - id: DAY
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Day of Image Acquisition
      2 BCS-NPI digits (DD), range 01-31.

  - id: HOUR
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Hour of Image Acquisition
      2 BCS-NPI digits (HH), range 00-23.

  - id: MINUTE
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Minute of Image Acquisition
      2 BCS-NPI digits (MM), range 00-59.

  - id: SECOND
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Second of Image Acquisition
      9 BCS-N real number (SS.SSSSSS), range 00.000000-59.999999.

  - id: NRG
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Number of Row Sections in RSM Grid
      8 BCS-NPI positive integer.

  - id: NCG
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Number of Column Sections in RSM Grid
      8 BCS-NPI positive integer.

  - id: TRG
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Total Number of Rows in RSM Ground Domain
      21 BCS-N real number.

  - id: TCG
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Total Number of Columns in RSM Ground Domain
      21 BCS-N real number.

  - id: GRNDD
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Ground Domain Form
      1 BCS-A character: 'G' = Geographic, 'R' = Rectangular.

  - id: XUOR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      X/Longitude Coordinate of Ground Reference Point Origin
      21 BCS-N real number.

  - id: YUOR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Y/Latitude Coordinate of Ground Reference Point Origin
      21 BCS-N real number.

  - id: ZUOR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Z/Height Coordinate of Ground Reference Point Origin
      21 BCS-N real number.

  - id: XUXR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector X Component for X/Longitude Axis
      21 BCS-N real number.

  - id: XUYR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Y Component for X/Longitude Axis
      21 BCS-N real number.

  - id: XUZR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Z Component for X/Longitude Axis
      21 BCS-N real number.

  - id: YUXR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector X Component for Y/Latitude Axis
      21 BCS-N real number.

  - id: YUYR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Y Component for Y/Latitude Axis
      21 BCS-N real number.

  - id: YUZR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Z Component for Y/Latitude Axis
      21 BCS-N real number.

  - id: ZUXR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector X Component for Z/Height Axis
      21 BCS-N real number.

  - id: ZUYR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Y Component for Z/Height Axis
      21 BCS-N real number.

  - id: ZUZR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Z Component for Z/Height Axis
      21 BCS-N real number.

  - id: V1X
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 1 X/Longitude Coordinate
      21 BCS-N real number.

  - id: V1Y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 1 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: V1Z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 1 Z/Height Coordinate
      21 BCS-N real number.

  - id: V2X
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 2 X/Longitude Coordinate
      21 BCS-N real number.

  - id: V2Y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 2 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: V2Z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 2 Z/Height Coordinate
      21 BCS-N real number.

  - id: V3X
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 3 X/Longitude Coordinate
      21 BCS-N real number.

  - id: V3Y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 3 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: V3Z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 3 Z/Height Coordinate
      21 BCS-N real number.

  - id: V4X
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 4 X/Longitude Coordinate
      21 BCS-N real number.

  - id: V4Y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 4 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: V4Z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 4 Z/Height Coordinate
      21 BCS-N real number.

  - id: V5X
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 5 X/Longitude Coordinate
      21 BCS-N real number.

  - id: V5Y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 5 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: V5Z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 5 Z/Height Coordinate
      21 BCS-N real number.

  - id: V6X
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 6 X/Longitude Coordinate
      21 BCS-N real number.

  - id: V6Y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 6 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: V6Z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 6 Z/Height Coordinate
      21 BCS-N real number.

  - id: V7X
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 7 X/Longitude Coordinate
      21 BCS-N real number.

  - id: V7Y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 7 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: V7Z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 7 Z/Height Coordinate
      21 BCS-N real number.

  - id: V8X
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 8 X/Longitude Coordinate
      21 BCS-N real number.

  - id: V8Y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 8 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: V8Z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 8 Z/Height Coordinate
      21 BCS-N real number.

  - id: GRPX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Reference Point X/Longitude Coordinate
      21 BCS-N real number.

  - id: GRPY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Reference Point Y/Latitude Coordinate
      21 BCS-N real number.

  - id: GRPZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Reference Point Z/Height Coordinate
      21 BCS-N real number.

  - id: FULLR
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Number of Rows in Full Image
      8 BCS-NPI positive integer.

  - id: FULLC
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Number of Columns in Full Image
      8 BCS-NPI positive integer.

  - id: MINR
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Minimum Row of Valid RSM
      8 BCS-NPI non-negative integer.

  - id: MAXR
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Maximum Row of Valid RSM
      8 BCS-NPI positive integer.

  - id: MINC
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Minimum Column of Valid RSM
      8 BCS-NPI non-negative integer.

  - id: MAXC
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Maximum Column of Valid RSM
      8 BCS-NPI positive integer.

  - id: IE0
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Elevation Angle Constant Coefficient
      21 BCS-N real number (degrees).

  - id: IER
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Elevation Angle Row Coefficient
      21 BCS-N real number (degrees/row).

  - id: IEC
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Elevation Angle Column Coefficient
      21 BCS-N real number (degrees/column).

  - id: IERR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Elevation Angle Row-Row Coefficient
      21 BCS-N real number.

  - id: IERC
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Elevation Angle Row-Column Coefficient
      21 BCS-N real number.

  - id: IECC
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Elevation Angle Column-Column Coefficient
      21 BCS-N real number.

  - id: IA0
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Azimuth Angle Constant Coefficient
      21 BCS-N real number (degrees).

  - id: IAR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Azimuth Angle Row Coefficient
      21 BCS-N real number (degrees/row).

  - id: IAC
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Azimuth Angle Column Coefficient
      21 BCS-N real number (degrees/column).

  - id: IARR
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Azimuth Angle Row-Row Coefficient
      21 BCS-N real number.

  - id: IARC
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Azimuth Angle Row-Column Coefficient
      21 BCS-N real number.

  - id: IACC
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Azimuth Angle Column-Column Coefficient
      21 BCS-N real number.

  - id: SPX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor X Position
      21 BCS-N real number (meters).

  - id: SVX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor X Velocity
      21 BCS-N real number (meters/second).

  - id: SAX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor X Acceleration
      21 BCS-N real number (meters/second^2).

  - id: SPY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor Y Position
      21 BCS-N real number (meters).

  - id: SVY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor Y Velocity
      21 BCS-N real number (meters/second).

  - id: SAY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor Y Acceleration
      21 BCS-N real number (meters/second^2).

  - id: SPZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor Z Position
      21 BCS-N real number (meters).

  - id: SVZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor Z Velocity
      21 BCS-N real number (meters/second).

  - id: SAZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor Z Acceleration
      21 BCS-N real number (meters/second^2).
