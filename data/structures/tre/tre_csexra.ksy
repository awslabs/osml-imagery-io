meta:
  id: tre_csexra
  title: Commercial Exploitation Reference Data TRE
  endian: be

doc: |
  CSEXRA TRE - Commercial Exploitation Reference Data
  
  Provides exploitation support data for commercial satellite imagery including:
  - Acquisition parameters
  - Environment conditions
  - Performance metrics
  - Multi-mate/stereo information
  - Processing history parameters
  
  This TRE is defined in STDI-0006 (NCDRD - NITF Commercial Dataset Requirements Document).
  The full specification is not publicly available and must be obtained from NGA.
  
  Reference: STDI-0006 (NCDRD), STDI-0002 Volume 1 Appendix S

seq:
  - id: sensor
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      Sensor Identifier
      6 BCS-A characters identifying the sensor.

  - id: time_first_line_image
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Time of First Line of Image
      12 BCS-N characters representing time in HHMMSS.SSSSS format.

  - id: time_image_duration
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Image Duration Time
      12 BCS-N characters representing duration in seconds.

  - id: max_gsd
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Maximum Ground Sample Distance
      5 BCS-N real number in meters.

  - id: along_scan_gsd
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Along Scan Ground Sample Distance
      5 BCS-N real number in meters.

  - id: cross_scan_gsd
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Cross Scan Ground Sample Distance
      5 BCS-N real number in meters.

  - id: geo_mean_gsd
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Geometric Mean Ground Sample Distance
      5 BCS-N real number in meters.

  - id: a_s_vert_gsd
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Along Scan Vertical GSD
      5 BCS-N real number in meters.

  - id: c_s_vert_gsd
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Cross Scan Vertical GSD
      5 BCS-N real number in meters.

  - id: geo_mean_vert_gsd
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Geometric Mean Vertical GSD
      5 BCS-N real number in meters.

  - id: geo_beta_angle
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Geometric Beta Angle
      5 BCS-N real number in degrees.

  - id: dynamic_range
    type: str
    size: 5
    encoding: BCS-NPI
    doc: |
      Dynamic Range
      5 BCS-NPI integer representing bits.

  - id: num_lines
    type: str
    size: 7
    encoding: BCS-NPI
    doc: |
      Number of Lines
      7 BCS-NPI integer.

  - id: num_samples
    type: str
    size: 5
    encoding: BCS-NPI
    doc: |
      Number of Samples
      5 BCS-NPI integer.

  - id: angle_to_north
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Angle to North
      7 BCS-N real number in degrees.

  - id: obliquity_angle
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Obliquity Angle
      6 BCS-N real number in degrees.

  - id: az_of_obliquity
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Azimuth of Obliquity
      7 BCS-N real number in degrees.

  - id: grp_id
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Ground Reference Point ID
      2 BCS-NPI integer.

  - id: grp_lat
    type: str
    size: 11
    encoding: BCS-N
    doc: |
      Ground Reference Point Latitude
      11 BCS-N real number in degrees.

  - id: grp_lon
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Ground Reference Point Longitude
      12 BCS-N real number in degrees.

  - id: grp_alt
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Ground Reference Point Altitude
      8 BCS-N real number in meters.

  - id: sun_azimuth
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Sun Azimuth Angle
      7 BCS-N real number in degrees.

  - id: sun_elevation
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Sun Elevation Angle
      7 BCS-N real number in degrees.

  - id: predicted_niirs
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Predicted NIIRS
      3 BCS-N real number (0.0-9.9).

  - id: circl_err
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Circular Error
      5 BCS-N real number in meters.

  - id: linear_err
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Linear Error
      5 BCS-N real number in meters.

  - id: cloud_cover
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Cloud Cover Percentage
      3 BCS-NPI integer (0-100).

  - id: rolling_shutter_flag
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Rolling Shutter Flag
      1 BCS-A character (Y/N).

  - id: ue_time_flag
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Uncertainty Estimate Time Flag
      1 BCS-A character (Y/N).

  - id: reserved_1
    type: str
    size: 14
    encoding: BCS-A
    doc: |
      Reserved Field 1
      14 BCS-A characters for future use.
