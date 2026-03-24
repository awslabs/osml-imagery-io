meta:
  id: tre_csexra
  title: Commercial Exploitation Reference Data TRE
  endian: be

doc: |
  CSEXRA TRE - Commercial Exploitation Reference Data (132 bytes)
  
  Provides exploitation support data for commercial satellite imagery including:
  - Acquisition parameters (sensor, timing, GSD)
  - Environment conditions (ground cover, snow depth, sun angles)
  - Performance metrics (predicted NIIRS, circular/linear error)
  
  This TRE is defined in STDI-0006 (NCDRD), Table 3.5-1.
  CEL = 00132.
  
  Reference: STDI-0006 (NCDRD) 18 February 2010, Section 3.5

seq:
  - id: SENSOR
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      Sensor Identifier
      Sensor associated with this instance of the TRE.
      Values: PAN, MS

  - id: TIME_FIRST_LINE_IMAGE
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Time of the First Line of Image (Synthetic Array)
      Time in seconds from midnight (UTC) for the first line,
      synthetic array, of the Dataset collection.
      Range: 00000.000000 to 86400.000000

  - id: TIME_IMAGE_DURATION
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Image Duration Time
      Time difference in seconds between the first line and last line
      (synthetic array). A preceding hyphen/minus (0x2D) indicates
      reverse chronological ordering of attitude, ephemeris, and image data.
      Range: -9999.999999 to 86400.000000

  - id: MAX_GSD
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Maximum Mean Ground Sample Distance
      Predicted maximum mean GSD for the primary target in inches.
      Range: 000.0 to 999.9

  - id: ALONG_SCAN_GSD
    type: str
    size: 5
    encoding: BCS-A
    doc: |
      Along Scan GSD
      Measured along scan ground sample distance in inches.
      Range: 000.0 to 999.9 or N/A

  - id: CROSS_SCAN_GSD
    type: str
    size: 5
    encoding: BCS-A
    doc: |
      Cross-Scan GSD
      Measured cross scan ground sample distance in inches.
      Range: 000.0 to 999.9 or N/A

  - id: GEO_MEAN_GSD
    type: str
    size: 5
    encoding: BCS-A
    doc: |
      Geometric Mean GSD
      Measured geometric mean ground sample distance in inches.
      Range: 000.0 to 999.9 or N/A

  - id: A_S_VERT_GSD
    type: str
    size: 5
    encoding: BCS-A
    doc: |
      Along Scan Vertical GSD
      Measured along scan vertical ground sample distance in inches.
      Range: 000.0 to 999.9 or N/A

  - id: C_S_VERT_GSD
    type: str
    size: 5
    encoding: BCS-A
    doc: |
      Cross-Scan Vertical GSD
      Measured cross scan vertical ground sample distance in inches.
      Range: 000.0 to 999.9 or N/A

  - id: GEO_MEAN_VERT_GSD
    type: str
    size: 5
    encoding: BCS-A
    doc: |
      Geometric Mean Vertical GSD
      Measured geometric mean vertical ground sample distance in inches.
      Range: 000.0 to 999.9 or N/A

  - id: GSD_BETA_ANGLE
    type: str
    size: 5
    encoding: BCS-A
    doc: |
      GSD Beta Angle
      Angle on ground (Earth tangent plane) between along scan
      and cross scan directions in degrees.
      Range: 00.0 to 180.0 or N/A

  - id: DYNAMIC_RANGE
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Dynamic Range
      Dynamic range extent of pixel values in the image.
      Range: 00000 to 02047

  - id: NUM_LINES
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Number of Lines
      Number of lines in the dataset (standard array) for the output product.
      Range: 0000101 to 9999999

  - id: NUM_SAMPLES
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Number of Samples
      Number of samples per line in the dataset for the output product.
      Range: 00101 to 99999

  - id: ANGLE_TO_NORTH
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Nominal Angle to True North
      Angle in degrees, measured clockwise, from the first row of
      the image to True North at Image Start Time.
      Range: 000.000 to 360.000

  - id: OBLIQUITY_ANGLE
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Nominal Obliquity Angle
      Obliquity angle measured from target local vertical in degrees.
      Range: 00.000 to 90.000

  - id: AZ_OF_OBLIQUITY
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Azimuth of Obliquity
      Azimuth of the target-SV line-of-sight vector projected in the
      target local horizontal plane, measured clockwise from True North,
      computed at Image Start Time. In degrees.
      Range: 000.000 to 360.000

  - id: GRD_COVER
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Ground Cover
      Snow or no snow indicator.
      1 = Snow, 0 = No Snow, 9 = Not Available

  - id: SNOW_DEPTH_CAT
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Snow Depth Category
      Weighted average of snow depth values for grids overlapping
      the tasked image area.
      0 = 0 inches, 1 = 1-8 inches or ice, 2 = 9-17 inches,
      3 = greater than 17 inches, 9 = Not Available

  - id: SUN_AZIMUTH
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Sun Azimuth Angle
      Azimuth of the target-sun line-of-sight vector projected in the
      target local horizontal plane, measured clockwise from True North,
      calculated at Image Start Time. In degrees.
      Range: 000.000 to 360.000

  - id: SUN_ELEVATION
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Sun Elevation Angle
      Sun elevation angle from the local target plane to the sun,
      calculated at Image Start Time. In degrees.
      Range: -90.000 to +90.000

  - id: PREDICTED_NIIRS
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Predicted NIIRS
      Imagery NIIRS value.
      Range: 0.0 to 9.0 or N/A

  - id: CIRCL_ERR
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Circular Error
      Predicted CE/90 geolocation error in the scene, in feet.
      Range: 000 to 999

  - id: LINEAR_ERR
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Linear Error
      Predicted LE/90 geolocation error in the scene, in feet.
      Range: 000 to 999
