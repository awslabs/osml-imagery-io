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
  - id: iid
    type: str
    size: 80
    encoding: BCS-A
    doc: |
      Image Identifier
      80 BCS-A characters identifying the image.

  - id: edition
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      RSM Image Support Data Edition
      40 BCS-A characters identifying the edition.

  - id: isid
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      Image Sequence Identifier
      40 BCS-A characters identifying the image sequence.

  - id: sid
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      Sensor Identifier
      40 BCS-A characters identifying the sensor.

  - id: stid
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      Sensor Type Identifier
      40 BCS-A characters identifying the sensor type.

  - id: year
    type: str
    size: 4
    encoding: BCS-NPI
    doc: |
      Year of Image Acquisition
      4 BCS-NPI digits (YYYY).

  - id: month
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Month of Image Acquisition
      2 BCS-NPI digits (MM), range 01-12.

  - id: day
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Day of Image Acquisition
      2 BCS-NPI digits (DD), range 01-31.

  - id: hour
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Hour of Image Acquisition
      2 BCS-NPI digits (HH), range 00-23.

  - id: minute
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Minute of Image Acquisition
      2 BCS-NPI digits (MM), range 00-59.

  - id: second
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Second of Image Acquisition
      9 BCS-N real number (SS.SSSSSS), range 00.000000-59.999999.

  - id: nrg
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Number of Row Sections in RSM Grid
      8 BCS-NPI positive integer.

  - id: ncg
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Number of Column Sections in RSM Grid
      8 BCS-NPI positive integer.

  - id: trg
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Total Number of Rows in RSM Ground Domain
      21 BCS-N real number.

  - id: tcg
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Total Number of Columns in RSM Ground Domain
      21 BCS-N real number.

  - id: grndd
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Ground Domain Form
      1 BCS-A character: 'G' = Geographic, 'R' = Rectangular.

  - id: xuor
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      X/Longitude Coordinate of Ground Reference Point Origin
      21 BCS-N real number.

  - id: yuor
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Y/Latitude Coordinate of Ground Reference Point Origin
      21 BCS-N real number.

  - id: zuor
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Z/Height Coordinate of Ground Reference Point Origin
      21 BCS-N real number.

  - id: xuxr
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector X Component for X/Longitude Axis
      21 BCS-N real number.

  - id: xuyr
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Y Component for X/Longitude Axis
      21 BCS-N real number.

  - id: xuzr
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Z Component for X/Longitude Axis
      21 BCS-N real number.

  - id: yuxr
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector X Component for Y/Latitude Axis
      21 BCS-N real number.

  - id: yuyr
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Y Component for Y/Latitude Axis
      21 BCS-N real number.

  - id: yuzr
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Z Component for Y/Latitude Axis
      21 BCS-N real number.

  - id: zuxr
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector X Component for Z/Height Axis
      21 BCS-N real number.

  - id: zuyr
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Y Component for Z/Height Axis
      21 BCS-N real number.

  - id: zuzr
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Unit Vector Z Component for Z/Height Axis
      21 BCS-N real number.

  - id: v1x
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 1 X/Longitude Coordinate
      21 BCS-N real number.

  - id: v1y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 1 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: v1z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 1 Z/Height Coordinate
      21 BCS-N real number.

  - id: v2x
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 2 X/Longitude Coordinate
      21 BCS-N real number.

  - id: v2y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 2 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: v2z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 2 Z/Height Coordinate
      21 BCS-N real number.

  - id: v3x
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 3 X/Longitude Coordinate
      21 BCS-N real number.

  - id: v3y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 3 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: v3z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 3 Z/Height Coordinate
      21 BCS-N real number.

  - id: v4x
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 4 X/Longitude Coordinate
      21 BCS-N real number.

  - id: v4y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 4 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: v4z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 4 Z/Height Coordinate
      21 BCS-N real number.

  - id: v5x
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 5 X/Longitude Coordinate
      21 BCS-N real number.

  - id: v5y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 5 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: v5z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 5 Z/Height Coordinate
      21 BCS-N real number.

  - id: v6x
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 6 X/Longitude Coordinate
      21 BCS-N real number.

  - id: v6y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 6 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: v6z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 6 Z/Height Coordinate
      21 BCS-N real number.

  - id: v7x
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 7 X/Longitude Coordinate
      21 BCS-N real number.

  - id: v7y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 7 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: v7z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 7 Z/Height Coordinate
      21 BCS-N real number.

  - id: v8x
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 8 X/Longitude Coordinate
      21 BCS-N real number.

  - id: v8y
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 8 Y/Latitude Coordinate
      21 BCS-N real number.

  - id: v8z
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Vertex 8 Z/Height Coordinate
      21 BCS-N real number.

  - id: grpx
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Reference Point X/Longitude Coordinate
      21 BCS-N real number.

  - id: grpy
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Reference Point Y/Latitude Coordinate
      21 BCS-N real number.

  - id: grpz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Reference Point Z/Height Coordinate
      21 BCS-N real number.

  - id: fullr
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Number of Rows in Full Image
      8 BCS-NPI positive integer.

  - id: fullc
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Number of Columns in Full Image
      8 BCS-NPI positive integer.

  - id: minr
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Minimum Row of Valid RSM
      8 BCS-NPI non-negative integer.

  - id: maxr
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Maximum Row of Valid RSM
      8 BCS-NPI positive integer.

  - id: minc
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Minimum Column of Valid RSM
      8 BCS-NPI non-negative integer.

  - id: maxc
    type: str
    size: 8
    encoding: BCS-NPI
    doc: |
      Maximum Column of Valid RSM
      8 BCS-NPI positive integer.

  - id: ie0
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Elevation Angle Constant Coefficient
      21 BCS-N real number (degrees).

  - id: ier
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Elevation Angle Row Coefficient
      21 BCS-N real number (degrees/row).

  - id: iec
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Elevation Angle Column Coefficient
      21 BCS-N real number (degrees/column).

  - id: ierr
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Elevation Angle Row-Row Coefficient
      21 BCS-N real number.

  - id: ierc
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Elevation Angle Row-Column Coefficient
      21 BCS-N real number.

  - id: iecc
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Elevation Angle Column-Column Coefficient
      21 BCS-N real number.

  - id: ia0
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Azimuth Angle Constant Coefficient
      21 BCS-N real number (degrees).

  - id: iar
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Azimuth Angle Row Coefficient
      21 BCS-N real number (degrees/row).

  - id: iac
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Azimuth Angle Column Coefficient
      21 BCS-N real number (degrees/column).

  - id: iarr
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Azimuth Angle Row-Row Coefficient
      21 BCS-N real number.

  - id: iarc
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Azimuth Angle Row-Column Coefficient
      21 BCS-N real number.

  - id: iacc
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Illumination Azimuth Angle Column-Column Coefficient
      21 BCS-N real number.

  - id: spx
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor X Position
      21 BCS-N real number (meters).

  - id: svx
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor X Velocity
      21 BCS-N real number (meters/second).

  - id: sax
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor X Acceleration
      21 BCS-N real number (meters/second^2).

  - id: spy
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor Y Position
      21 BCS-N real number (meters).

  - id: svy
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor Y Velocity
      21 BCS-N real number (meters/second).

  - id: say
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor Y Acceleration
      21 BCS-N real number (meters/second^2).

  - id: spz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor Z Position
      21 BCS-N real number (meters).

  - id: svz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor Z Velocity
      21 BCS-N real number (meters/second).

  - id: saz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Sensor Z Acceleration
      21 BCS-N real number (meters/second^2).
