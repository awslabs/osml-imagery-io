meta:
  id: tre_csexrb
  title: Common Sensor Exploitation Reference Data TRE
  endian: be

doc: |
  CSEXRB TRE - Common Sensor Exploitation Reference Data
  Version 1.2
  
  Part of the GLAS/GFM (Generic Linear Array Scanner / Generic Frame-sequence Model)
  support data extensions. Contains the date of image acquisition, time tags associated
  with exposure of a specific line of an image or frame, number of lines and samples
  in the collected image, and Universally Unique Identifiers (UUIDs) to associate
  image segments containing GLAS/GFM TREs with GLAS/GFM DESs in the same NITF file.
  
  This is a highly complex TRE with many conditional fields based on SENSOR_TYPE
  (S=scanner, F=framer) and TIME_STAMP_LOC values. It also includes optional
  reserved field areas for collection geometry, target information, collection
  criteria, and quality metrics.
  
  Reference: STDI-0002 Volume 2, Appendix M - GLAS-GFM

seq:
  # Core identification fields
  - id: image_uuid
    type: str
    size: 36
    encoding: BCS-A
    doc: |
      UUID Assigned to the Current Image Plane
      A valid UUID string in canonical format (e.g., dbe26dc7-e003-4d29-8edb-41acc0e86b6e).
      36 BCS-A characters.

  - id: num_assoc_des
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Number of GLAS/GFM DESs Associated with this Image
      If CSEXRB provides high level exploitation metadata and does not support
      a GLAS/GFM data model, then NUM_ASSOC_DES = 0.
      3 BCS-N integer, range 000-999.

  - id: assoc_des_uuids
    type: str
    size: 36
    encoding: BCS-A
    repeat: expr
    repeat-expr: num_assoc_des.to_i
    if: num_assoc_des.to_i > 0
    doc: |
      UUIDs of Associated GLAS/GFM DESs
      Each UUID identifies a GLAS/GFM DES associated with the current image.
      36 BCS-A characters per UUID.

  # Platform/Sensor identification
  - id: platform_id
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      Platform Identifier
      Identifier of the system that collected the current image.
      6 BCS-A characters.

  - id: payload_id
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      Payload Identifier
      Identifier of the payload that collected the current image.
      6 BCS-A characters.

  - id: sensor_id
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      Sensor Identifier
      Identifier of the sensor that collected the current image.
      6 BCS-A characters.

  - id: sensor_type
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Sensor Type
      S = line scanner, F = framing sensor, space = N/A.
      1 BCS-A character.

  # Ground reference point (ECF coordinates)
  - id: ground_ref_point_x
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Ground Reference Point X Coordinate (ECF)
      12 BCS-A, range -99999999.99 to +99999999.99 meters, or BCS spaces.

  - id: ground_ref_point_y
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Ground Reference Point Y Coordinate (ECF)
      12 BCS-A, range -99999999.99 to +99999999.99 meters, or BCS spaces.

  - id: ground_ref_point_z
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Ground Reference Point Z Coordinate (ECF)
      12 BCS-A, range -99999999.99 to +99999999.99 meters, or BCS spaces.

  # Scanner-specific fields (SENSOR_TYPE = S)
  - id: scanner_data
    type: scanner_data_t
    if: sensor_type == "S"

  # Framer-specific fields (SENSOR_TYPE = F)
  - id: framer_data
    type: framer_data_t
    if: sensor_type == "F"

  # GSD and geometry fields
  - id: max_gsd
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Maximum Mean Ground Sample Distance
      12 BCS-A, range 0000000000.0 to 9999999999.9 inches, or BCS spaces.

  - id: along_scan_gsd
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Measured Along-Scan GSD
      12 BCS-A, range 0000000000.0 to 9999999999.9 inches, or BCS spaces.

  - id: cross_scan_gsd
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Measured Cross-Scan GSD
      12 BCS-A, range 0000000000.0 to 9999999999.9 inches, or BCS spaces.

  - id: geo_mean_gsd
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Measured Geometric Mean GSD
      12 BCS-A, range 0000000000.0 to 9999999999.9 inches, or BCS spaces.

  - id: a_s_vert_gsd
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Measured Along-Scan Vertical GSD
      12 BCS-A, range 0000000000.0 to 9999999999.9 inches, or BCS spaces.

  - id: c_s_vert_gsd
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Measured Cross-Scan Vertical GSD
      12 BCS-A, range 0000000000.0 to 9999999999.9 inches, or BCS spaces.

  - id: geo_mean_vert_gsd
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Measured Geometric Mean Vertical GSD
      12 BCS-A, range 0000000000.0 to 9999999999.9 inches, or BCS spaces.

  - id: gsd_beta_angle
    type: str
    size: 5
    encoding: BCS-A
    doc: |
      Angle Between Along-Scan and Cross-Scan Directions
      5 BCS-A, range 000.0 to 180.0 degrees, or BCS spaces.

  - id: dynamic_range
    type: str
    size: 5
    encoding: BCS-A
    doc: |
      Dynamic Range of Pixels in Image Across All Bands
      5 BCS-A, range 00000 to 99999 digital numbers, or BCS spaces.

  # Image dimensions
  - id: num_lines
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Number of Lines in the Entire Image
      7 BCS-N integer, range 0000000 to 9999999 lines.

  - id: num_samples
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Number of Samples Per Line in the Entire Image
      5 BCS-N integer, range 00000 to 99999 samples.

  # Geometry angles
  - id: angle_to_north
    type: str
    size: 7
    encoding: BCS-A
    doc: |
      Angle to True North
      7 BCS-A, range 000.000 to 359.999 degrees, or BCS spaces.

  - id: obliquity_angle
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      Obliquity Angle
      6 BCS-A, range 00.000 to 90.000 degrees, or BCS spaces.

  - id: az_of_obliquity
    type: str
    size: 7
    encoding: BCS-A
    doc: |
      Azimuth of Obliquity
      7 BCS-A, range 000.000 to 359.999 degrees, or BCS spaces.

  # Correction flags
  - id: atm_refr_flag
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Atmospheric Refraction Flag
      0 = Do not apply correction, 1 = Apply correction.
      1 BCS-N integer.

  - id: vel_aber_flag
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Velocity Aberration Flag
      0 = Do not apply correction, 1 = Apply correction.
      1 BCS-N integer.

  # Environmental metadata
  - id: grd_cover
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Ground Cover Flag
      1 = Snow, 0 = No Snow, 9 = Not Available.
      1 BCS-N integer.

  - id: snow_depth_category
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Snow Depth Category
      0 = 0 inches, 1 = 1-8 inches, 2 = 9-17 inches, 3 = >17 inches, 9 = Not Available.
      1 BCS-N integer.

  - id: sun_azimuth
    type: str
    size: 7
    encoding: BCS-A
    doc: |
      Sun Azimuth Angle
      7 BCS-A, range 000.000 to 359.999 degrees, or BCS spaces.

  - id: sun_elevation
    type: str
    size: 7
    encoding: BCS-A
    doc: |
      Sun Elevation Angle
      7 BCS-A, range -90.000 to +90.000 degrees, or BCS spaces.

  # Performance metadata
  - id: predicted_niirs
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Predicted NIIRS
      3 BCS-A, range 0.0 to 9.0 NIIRS, or BCS spaces.

  - id: circl_err
    type: str
    size: 5
    encoding: BCS-A
    doc: |
      Circular Error (CE90)
      5 BCS-A, range 000.0 to 999.9 feet, or BCS spaces.

  - id: linear_err
    type: str
    size: 5
    encoding: BCS-A
    doc: |
      Linear Error (LE90)
      5 BCS-A, range 000.0 to 999.9 feet, or BCS spaces.

  - id: cloud_cover
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Cloud Cover Percentage
      3 BCS-A, range 000 to 100 percent, 999 = unknown, or BCS spaces.

  # Framer rolling shutter flag (SENSOR_TYPE = F only)
  - id: rolling_shutter_flag
    type: str
    size: 1
    encoding: BCS-A
    if: sensor_type == "F"
    doc: |
      Rolling Shutter Flag (conditional: SENSOR_TYPE = F)
      0 = same integration time across frame, 1 = changing time, space = N/A.
      1 BCS-A character.

  # Time unmodeled error flag
  - id: ue_time_flag
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Time Unmodeled Error Flag
      0 = no, 1 = yes, space = N/A.
      1 BCS-A character.

  # Reserved field areas
  - id: reserved_len
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Length of Reserved Field Areas
      Total bytes of all fields in all Reserved Field Areas.
      5 BCS-N integer, 00000 or 00063 to maximum allowed.

  - id: reserved_data
    size: reserved_len.to_i
    if: reserved_len.to_i > 0
    doc: |
      Reserved Field Areas
      Contains optional collection geometry, target information,
      collection criteria, and quality metrics data.
      Variable length based on RESERVED_LEN.

types:
  scanner_data_t:
    doc: |
      Scanner-Specific Data (SENSOR_TYPE = S)
      Contains timing information for line scanner sensors.
    seq:
      - id: day_first_line_image
        type: str
        size: 8
        encoding: BCS-N
        doc: |
          Day of First Line of the Synthetic Array Image
          8 BCS-N in CCYYMMDD format (UTC Zulu).

      - id: time_first_line_image
        type: str
        size: 15
        encoding: BCS-N
        doc: |
          Time of First Line of the Image
          Seconds from midnight to start of collection of first line.
          15 BCS-N real number, range 00000.000000000 to 86399.999999999 seconds (UTC Zulu).

      - id: time_image_duration
        type: str
        size: 16
        encoding: BCS-N
        doc: |
          Image Duration Time
          Signed time difference between start collection times for top and bottom lines.
          16 BCS-N real number, range -86399.999999999 to +86399.999999999 seconds.

  framer_data_t:
    doc: |
      Framer-Specific Data (SENSOR_TYPE = F)
      Contains timing information for framing sensors.
    seq:
      - id: time_stamp_loc
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Location of Frame Time Stamps
          0 = values in this CSEXRB TRE, 1 = values in MTIMSA TRE.
          1 BCS-N integer.

      - id: frame_timing_data
        type: frame_timing_data_t
        if: time_stamp_loc == "0"

  frame_timing_data_t:
    doc: |
      Frame Timing Data (TIME_STAMP_LOC = 0)
      Contains detailed frame timing information when timestamps are in CSEXRB.
    seq:
      - id: reference_frame_num
        type: str
        size: 9
        encoding: BCS-A
        doc: |
          Reference Frame Number
          Absolute frame number of the first frame of this temporal block.
          9 BCS-A, range 000000001 to 999999999, or BCS spaces.

      - id: base_timestamp
        type: str
        size: 24
        encoding: BCS-N
        doc: |
          Base Time Stamp
          Base time stamp from which frame time stamps are derived.
          24 BCS-N in CCYYMMDDhhmmss.nnnnnnnnn format (UTC Zulu).

      - id: dt_multiplier
        type: u8be
        doc: |
          Delta Time Duration
          Number of nanoseconds equal to one "time unit".
          8-byte unsigned integer (UINT64), range 1 to 2^64-1.

      - id: dt_size
        type: u1
        doc: |
          Byte Size of the Delta Time Values
          Size in bytes of the DTn values.
          1-byte unsigned integer (UINT8), range 1-8.

      - id: number_frames
        type: u4be
        doc: |
          Number of Frames in the Current Temporal Block
          Number of frames in this image segment for this camera and temporal block.
          4-byte unsigned integer (UINT32), range 1 to 2^32-1.

      - id: number_dt
        type: u4be
        doc: |
          Number of Delta Time Values
          Number of delta time unit (DTn) values contained in this NITF image segment.
          If NUMBER_DT = 0, no DTn values are present (single frame case).
          4-byte unsigned integer (UINT32), range 0 to 2^32-1.

      - id: dt_values
        size: dt_size
        repeat: expr
        repeat-expr: number_dt
        if: number_dt > 0
        doc: |
          Delta Time Values (DTn)
          Number of delta time units between this frame and the previous frame.
          Variable size unsigned integers based on DT_SIZE.
