meta:
  id: tre_sensra
  title: Sensor Parameters TRE (Legacy)
  endian: be

doc: |
  SENSRA TRE - Sensor Parameters Tagged Record Extension (Legacy/Inactive)
  
  This TRE has been superseded by SENSRB and is marked as INACTIVE.
  It is retained for legacy support only. New implementations should use SENSRB.
  
  SENSRA provides basic sensor parameters for imaging electro-optical sensors
  including position, attitude, and velocity information.
  
  Total length: 132 bytes
  
  Reference: STDI-0002 Volume 1, Appendix Z - SENSRB (Section Z.6.1 SENSRA to SENSRB Mapping)

seq:
  - id: ref_row
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Reference Row
      8 BCS-N integer representing the reference pixel row.

  - id: ref_col
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Reference Column
      8 BCS-N integer representing the reference pixel column.

  - id: sensor_model
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      Sensor Model
      6 BCS-A characters identifying the sensor model.

  - id: sensor_mount
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Sensor Mount Type
      3 BCS-A characters describing the sensor mount.
      Not supported in SENSRB.

  - id: sensor_loc
    type: str
    size: 21
    encoding: BCS-A
    doc: |
      Sensor Location
      21 BCS-A characters representing geodetic coordinates (lat/lon).
      Format: ±DD.DDDDDD±DDD.DDDDDD

  - id: sensor_alt_source
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Sensor Altitude Source
      1 BCS-A character indicating altitude datum.
      B = MSL, G = HAE, R = AGL, M = not applicable.

  - id: sensor_alt
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Sensor Altitude
      6 BCS-N real number representing sensor altitude.

  - id: sensor_alt_unit
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Sensor Altitude Unit
      1 BCS-A character indicating unit.
      f = feet, m = meters.

  - id: sensor_agl
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Sensor Above Ground Level
      5 BCS-N real number representing height above ground.

  - id: sensor_pitch
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Sensor Pitch Angle
      7 BCS-N real number in degrees.
      Note: SENSRA angle definitions differ from SENSRB Euler angles.

  - id: sensor_roll
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Sensor Roll Angle
      8 BCS-N real number in degrees.
      Note: SENSRA angle definitions differ from SENSRB Euler angles.

  - id: sensor_yaw
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Sensor Yaw Angle
      8 BCS-N real number in degrees.
      Note: SENSRA angle definitions differ from SENSRB Euler angles.

  - id: platform_pitch
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Platform Pitch Angle
      7 BCS-N real number in degrees.

  - id: platform_roll
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Platform Roll Angle
      8 BCS-N real number in degrees.

  - id: platform_hdg
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Platform Heading
      5 BCS-N real number in degrees (0-360).

  - id: ground_spd_source
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Ground Speed Source
      1 BCS-A character indicating speed source.

  - id: ground_spd
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Ground Speed
      6 BCS-N real number representing ground speed.

  - id: ground_spd_unit
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Ground Speed Unit
      1 BCS-A character indicating unit.
      f = feet/sec, m = meters/sec, k = knots.

  - id: ground_track
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Ground Track Angle
      5 BCS-N real number in degrees (0-360).

  - id: vert_vel
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Vertical Velocity
      5 BCS-N real number. Positive is upward.
      Note: SENSRB uses positive downward convention.

  - id: vert_vel_unit
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Vertical Velocity Unit
      1 BCS-A character indicating unit per minute.

  - id: swath_frames
    type: str
    size: 4
    encoding: BCS-NPI
    doc: |
      Swath Frames
      4 BCS-NPI integer.

  - id: n_swaths
    type: str
    size: 4
    encoding: BCS-NPI
    doc: |
      Number of Swaths
      4 BCS-NPI integer.

  - id: spot_num
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Spot Number
      3 BCS-NPI integer.
