meta:
  id: tre_sensrb
  title: General Electro-Optical Sensor Parameters TRE
  endian: be

doc: |
  SENSRB TRE - General Electro-Optical Sensor Parameters
  Version 2.2
  
  Provides sensor parameters for imaging electro-optical (EO) sensors including
  visible, infrared, multi- and hyperspectral sensors. Contains 15 conditional
  modules for sensor identification, array parameters, calibration, image formation,
  position, attitude, velocity, point sets, time-stamped data, pixel-referenced data,
  uncertainty data, and additional parameters.
  
  Reference: STDI-0002 Volume 1, Appendix Z - SENSRB

seq:
  # Module 01: General Data
  - id: general_data
    type: str
    size: 1
    encoding: BCS-A
    doc: General Data Flag (Y/N)

  - id: general_data_module
    type: general_data_module_t
    if: general_data == "Y"

  # Module 02: Sensor Array Data
  - id: sensor_array_data
    type: str
    size: 1
    encoding: BCS-A
    doc: Sensor Array Data Flag (Y/N)

  - id: sensor_array_module
    type: sensor_array_module_t
    if: sensor_array_data == "Y"

  # Module 03: Sensor Calibration Data
  - id: sensor_calibration_data
    type: str
    size: 1
    encoding: BCS-A
    doc: Sensor Calibration Data Flag (Y/N)

  - id: sensor_calibration_module
    type: sensor_calibration_module_t
    if: sensor_calibration_data == "Y"

  # Module 04: Image Formation Data
  - id: image_formation_data
    type: str
    size: 1
    encoding: BCS-A
    doc: Image Formation Data Flag (Y/N)

  - id: image_formation_module
    type: image_formation_module_t
    if: image_formation_data == "Y"

  # Module 05: Reference Time/Pixel
  - id: reference_time
    type: str
    size: 12
    encoding: BCS-N
    doc: Reference Time of Applicability (seconds relative to START_TIME)

  - id: reference_row
    type: str
    size: 8
    encoding: BCS-N
    doc: Reference Pixel Row of Applicability

  - id: reference_column
    type: str
    size: 8
    encoding: BCS-N
    doc: Reference Pixel Column of Applicability

  # Module 06: Sensor Position Data (Required)
  - id: latitude_or_x
    type: str
    size: 11
    encoding: BCS-N
    doc: Sensor/Platform Latitude or ECEF X Position

  - id: longitude_or_y
    type: str
    size: 12
    encoding: BCS-N
    doc: Sensor/Platform Longitude or ECEF Y Position

  - id: altitude_or_z
    type: str
    size: 11
    encoding: BCS-N
    doc: Sensor/Platform Altitude or ECEF Z Position

  - id: sensor_x_offset
    type: str
    size: 8
    encoding: BCS-N
    doc: Sensor X Position Offset Relative to Platform

  - id: sensor_y_offset
    type: str
    size: 8
    encoding: BCS-N
    doc: Sensor Y Position Offset Relative to Platform

  - id: sensor_z_offset
    type: str
    size: 8
    encoding: BCS-N
    doc: Sensor Z Position Offset Relative to Platform

  # Module 07: Attitude Euler Angles
  - id: attitude_euler_angles
    type: str
    size: 1
    encoding: BCS-A
    doc: Attitude Euler Angles Flag (Y/N)

  - id: attitude_euler_module
    type: attitude_euler_module_t
    if: attitude_euler_angles == "Y"

  # Module 08: Attitude Unit Vectors
  - id: attitude_unit_vectors
    type: str
    size: 1
    encoding: BCS-A
    doc: Attitude Unit Vectors Flag (Y/N)

  - id: attitude_unit_vectors_module
    type: attitude_unit_vectors_module_t
    if: attitude_unit_vectors == "Y"

  # Module 09: Attitude Quaternion
  - id: attitude_quaternion
    type: str
    size: 1
    encoding: BCS-A
    doc: Attitude Quaternion Flag (Y/N)

  - id: attitude_quaternion_module
    type: attitude_quaternion_module_t
    if: attitude_quaternion == "Y"

  # Module 10: Sensor Velocity Data
  - id: sensor_velocity_data
    type: str
    size: 1
    encoding: BCS-A
    doc: Sensor Velocity Data Flag (Y/N)

  - id: sensor_velocity_module
    type: sensor_velocity_module_t
    if: sensor_velocity_data == "Y"

  # Module 11: Point Set Data
  - id: point_set_data
    type: str
    size: 2
    encoding: BCS-NPI
    doc: Number of Point Sets (00-99)

  - id: point_sets
    type: point_set_t
    repeat: expr
    repeat-expr: point_set_data.to_i
    if: point_set_data.to_i > 0

  # Module 12: Time Stamped Data Sets
  - id: time_stamped_data_sets
    type: str
    size: 2
    encoding: BCS-NPI
    doc: Number of Time Stamped Data Sets (00-99)

  - id: time_stamped_sets
    type: time_stamped_set_t
    repeat: expr
    repeat-expr: time_stamped_data_sets.to_i
    if: time_stamped_data_sets.to_i > 0

  # Module 13: Pixel Referenced Data Sets
  - id: pixel_referenced_data_sets
    type: str
    size: 2
    encoding: BCS-NPI
    doc: Number of Pixel Referenced Data Sets (00-99)

  - id: pixel_referenced_sets
    type: pixel_referenced_set_t
    repeat: expr
    repeat-expr: pixel_referenced_data_sets.to_i
    if: pixel_referenced_data_sets.to_i > 0

  # Module 14: Uncertainty Data
  - id: uncertainty_data
    type: str
    size: 3
    encoding: BCS-NPI
    doc: Number of Uncertainty Data Sets (000-999)

  - id: uncertainty_sets
    type: uncertainty_set_t
    repeat: expr
    repeat-expr: uncertainty_data.to_i
    if: uncertainty_data.to_i > 0

  # Module 15: Additional Parameter Data
  - id: additional_parameter_data
    type: str
    size: 3
    encoding: BCS-NPI
    doc: Number of Additional Parameters (000-999)

  - id: additional_parameters
    type: additional_parameter_t
    repeat: expr
    repeat-expr: additional_parameter_data.to_i
    if: additional_parameter_data.to_i > 0

types:
  general_data_module_t:
    seq:
      - id: sensor
        type: str
        size: 25
        encoding: BCS-A
        doc: Sensor Registered Name or Model

      - id: sensor_uri
        type: str
        size: 32
        encoding: BCS-A
        doc: Sensor Uniform Resource Identifier

      - id: platform
        type: str
        size: 25
        encoding: BCS-A
        doc: Platform Common Name

      - id: platform_uri
        type: str
        size: 32
        encoding: BCS-A
        doc: Platform Uniform Resource Identifier

      - id: operation_domain
        type: str
        size: 10
        encoding: BCS-A
        doc: Operational Domain (Airborne, Spaceborne, Waterborne, Ground)

      - id: content_level
        type: str
        size: 1
        encoding: BCS-NPI
        doc: Content Level (0-9)

      - id: geodetic_system
        type: str
        size: 5
        encoding: BCS-A
        doc: Geodetic Reference System (default WGS84)

      - id: geodetic_type
        type: str
        size: 1
        encoding: BCS-A
        doc: Geodetic Coordinate Type (G=Geographic, C=Geocentric)

      - id: elevation_datum
        type: str
        size: 3
        encoding: BCS-A
        doc: Elevation/Altitude Datum (HAE, MSL, AGL)

      - id: length_unit
        type: str
        size: 2
        encoding: BCS-A
        doc: Length Unit System (SI or EE)

      - id: angular_unit
        type: str
        size: 3
        encoding: BCS-A
        doc: Angular Unit Type (DEG, RAD, SMC)

      - id: start_date
        type: str
        size: 8
        encoding: BCS-NI
        doc: Imaging Start Date (YYYYMMDD)

      - id: start_time
        type: str
        size: 14
        encoding: BCS-N
        doc: Imaging Start Time (seconds into day)

      - id: end_date
        type: str
        size: 8
        encoding: BCS-NI
        doc: Imaging End Date (YYYYMMDD)

      - id: end_time
        type: str
        size: 14
        encoding: BCS-N
        doc: Imaging End Time (seconds into day)

      - id: generation_count
        type: str
        size: 2
        encoding: BCS-NPI
        doc: Generation Count (00-99)

      - id: generation_date
        type: str
        size: 8
        encoding: BCS-NI
        doc: Generation Date (YYYYMMDD)

      - id: generation_time
        type: str
        size: 10
        encoding: BCS-N
        doc: Generation Time (HHMMSS.sss)

  sensor_array_module_t:
    seq:
      - id: detection
        type: str
        size: 20
        encoding: BCS-A
        doc: Detection Type

      - id: row_detectors
        type: str
        size: 8
        encoding: BCS-NPI
        doc: Number of Detector Rows

      - id: column_detectors
        type: str
        size: 8
        encoding: BCS-NPI
        doc: Number of Detector Columns

      - id: row_metric
        type: str
        size: 8
        encoding: BCS-N
        doc: Physical Dimension of Used Rows (cm or in)

      - id: column_metric
        type: str
        size: 8
        encoding: BCS-N
        doc: Physical Dimension of Used Columns (cm or in)

      - id: focal_length
        type: str
        size: 8
        encoding: BCS-N
        doc: Best Known Focal Length (cm or in)

      - id: row_fov
        type: str
        size: 8
        encoding: BCS-N
        doc: Field of View - Rows (deg, rad, or smc)

      - id: column_fov
        type: str
        size: 8
        encoding: BCS-N
        doc: Field of View - Columns (deg, rad, or smc)

      - id: calibrated
        type: str
        size: 1
        encoding: BCS-A
        doc: Focal Length Calibration Flag (Y/N)

  sensor_calibration_module_t:
    seq:
      - id: calibration_unit
        type: str
        size: 2
        encoding: BCS-A
        doc: Calibration Unit System (mm or px)

      - id: principal_point_offset_x
        type: str
        size: 9
        encoding: BCS-N
        doc: Principal Point Offset X

      - id: principal_point_offset_y
        type: str
        size: 9
        encoding: BCS-N
        doc: Principal Point Offset Y

      - id: radial_distort_1
        type: str
        size: 12
        encoding: BCS-A
        doc: First Radial Distortion Coefficient (k1)

      - id: radial_distort_2
        type: str
        size: 12
        encoding: BCS-A
        doc: Second Radial Distortion Coefficient (k2)

      - id: radial_distort_3
        type: str
        size: 12
        encoding: BCS-A
        doc: Third Radial Distortion Coefficient (k3)

      - id: radial_distort_limit
        type: str
        size: 9
        encoding: BCS-N
        doc: Limit of Radial Distortion Fit

      - id: decent_distort_1
        type: str
        size: 12
        encoding: BCS-A
        doc: First Decentering Distortion Coefficient (p1)

      - id: decent_distort_2
        type: str
        size: 12
        encoding: BCS-A
        doc: Second Decentering Distortion Coefficient (p2)

      - id: affinity_distort_1
        type: str
        size: 12
        encoding: BCS-A
        doc: First Affinity Distortion Coefficient (b1)

      - id: affinity_distort_2
        type: str
        size: 12
        encoding: BCS-A
        doc: Second Affinity Distortion Coefficient (b2)

      - id: calibration_date
        type: str
        size: 8
        encoding: BCS-NI
        doc: Calibration Report Date (YYYYMMDD)


  image_formation_module_t:
    seq:
      - id: method
        type: str
        size: 15
        encoding: BCS-A
        doc: Image Formation Method (Single Frame, Continuous, etc.)

      - id: mode
        type: str
        size: 3
        encoding: BCS-A
        doc: Imaging Mode (PAN, MS, HS, etc.)

      - id: row_count
        type: str
        size: 8
        encoding: BCS-NPI
        doc: Number of Image Rows

      - id: column_count
        type: str
        size: 8
        encoding: BCS-NPI
        doc: Number of Image Columns

      - id: row_set
        type: str
        size: 8
        encoding: BCS-NPI
        doc: Row Set (first row of image in detector array)

      - id: column_set
        type: str
        size: 8
        encoding: BCS-NPI
        doc: Column Set (first column of image in detector array)

      - id: row_rate
        type: str
        size: 10
        encoding: BCS-N
        doc: Row Rate (rows per second)

      - id: column_rate
        type: str
        size: 10
        encoding: BCS-N
        doc: Column Rate (columns per second)

      - id: first_pixel_row
        type: str
        size: 8
        encoding: BCS-NPI
        doc: First Pixel Row

      - id: first_pixel_column
        type: str
        size: 8
        encoding: BCS-NPI
        doc: First Pixel Column

      - id: transform_params
        type: str
        size: 1
        encoding: BCS-NPI
        doc: Number of Transform Parameters (0-6)

      - id: transform_param_values
        type: str
        size: 12
        encoding: BCS-N
        repeat: expr
        repeat-expr: transform_params.to_i
        if: transform_params.to_i > 0

  attitude_euler_module_t:
    seq:
      - id: sensor_angle_model
        type: str
        size: 1
        encoding: BCS-NPI
        doc: Sensor Angle Model (1-4)

      - id: sensor_angle_1
        type: str
        size: 10
        encoding: BCS-N
        doc: Sensor Angle 1 (deg, rad, or smc)

      - id: sensor_angle_2
        type: str
        size: 9
        encoding: BCS-N
        doc: Sensor Angle 2 (deg, rad, or smc)

      - id: sensor_angle_3
        type: str
        size: 10
        encoding: BCS-N
        doc: Sensor Angle 3 (deg, rad, or smc)

      - id: platform_relative
        type: str
        size: 1
        encoding: BCS-A
        doc: Platform Relative Flag (Y/N)

      - id: platform_heading
        type: str
        size: 9
        encoding: BCS-N
        doc: Platform Heading (deg, rad, or smc)

      - id: platform_pitch
        type: str
        size: 9
        encoding: BCS-N
        doc: Platform Pitch (deg, rad, or smc)

      - id: platform_roll
        type: str
        size: 10
        encoding: BCS-N
        doc: Platform Roll (deg, rad, or smc)

  attitude_unit_vectors_module_t:
    seq:
      - id: icx_north_or_x
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column X Unit Vector - North or X Component

      - id: icx_east_or_y
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column X Unit Vector - East or Y Component

      - id: icx_down_or_z
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column X Unit Vector - Down or Z Component

      - id: icy_north_or_x
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column Y Unit Vector - North or X Component

      - id: icy_east_or_y
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column Y Unit Vector - East or Y Component

      - id: icy_down_or_z
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column Y Unit Vector - Down or Z Component

      - id: icz_north_or_x
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column Z Unit Vector - North or X Component

      - id: icz_east_or_y
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column Z Unit Vector - East or Y Component

      - id: icz_down_or_z
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column Z Unit Vector - Down or Z Component

  attitude_quaternion_module_t:
    seq:
      - id: attitude_q1
        type: str
        size: 10
        encoding: BCS-N
        doc: Attitude Quaternion Component Q1

      - id: attitude_q2
        type: str
        size: 10
        encoding: BCS-N
        doc: Attitude Quaternion Component Q2

      - id: attitude_q3
        type: str
        size: 10
        encoding: BCS-N
        doc: Attitude Quaternion Component Q3

      - id: attitude_q4
        type: str
        size: 10
        encoding: BCS-N
        doc: Attitude Quaternion Component Q4

  sensor_velocity_module_t:
    seq:
      - id: velocity_north_or_x
        type: str
        size: 9
        encoding: BCS-N
        doc: Sensor Velocity - North or X Component (m/s or ft/s)

      - id: velocity_east_or_y
        type: str
        size: 9
        encoding: BCS-N
        doc: Sensor Velocity - East or Y Component (m/s or ft/s)

      - id: velocity_down_or_z
        type: str
        size: 9
        encoding: BCS-N
        doc: Sensor Velocity - Down or Z Component (m/s or ft/s)

  point_set_t:
    seq:
      - id: point_set_type
        type: str
        size: 25
        encoding: BCS-A
        doc: Point Set Type Description

      - id: point_count
        type: str
        size: 3
        encoding: BCS-NPI
        doc: Number of Points in Set (001-999)

      - id: points
        type: point_t
        repeat: expr
        repeat-expr: point_count.to_i

  point_t:
    seq:
      - id: row
        type: str
        size: 8
        encoding: BCS-N
        doc: Point Row Location

      - id: column
        type: str
        size: 8
        encoding: BCS-N
        doc: Point Column Location

      - id: latitude
        type: str
        size: 10
        encoding: BCS-N
        doc: Point Latitude

      - id: longitude
        type: str
        size: 11
        encoding: BCS-N
        doc: Point Longitude

      - id: elevation
        type: str
        size: 6
        encoding: BCS-N
        doc: Point Elevation

      - id: range
        type: str
        size: 8
        encoding: BCS-N
        doc: Point Range

  time_stamped_set_t:
    seq:
      - id: time_stamp_type
        type: str
        size: 3
        encoding: BCS-A
        doc: Time Stamp Type (06b, 06c, 07a, 07b, 07c, 08a, 08b, 08c, 09a, 10a)

      - id: time_stamp_count
        type: str
        size: 4
        encoding: BCS-NPI
        doc: Number of Time Stamps (0001-9999)

      - id: time_stamps
        type: time_stamp_t
        repeat: expr
        repeat-expr: time_stamp_count.to_i

  time_stamp_t:
    seq:
      - id: time_stamp_time
        type: str
        size: 12
        encoding: BCS-N
        doc: Time Stamp Time (seconds relative to START_TIME)

      - id: time_stamp_value
        type: str
        size: 12
        encoding: BCS-N
        doc: Time Stamp Value

  pixel_referenced_set_t:
    seq:
      - id: pixel_reference_type
        type: str
        size: 3
        encoding: BCS-A
        doc: Pixel Reference Type (06b, 06c, 07a, 07b, 07c, 08a, 08b, 08c, 09a, 10a)

      - id: pixel_reference_count
        type: str
        size: 4
        encoding: BCS-NPI
        doc: Number of Pixel References (0001-9999)

      - id: pixel_references
        type: pixel_reference_t
        repeat: expr
        repeat-expr: pixel_reference_count.to_i

  pixel_reference_t:
    seq:
      - id: pixel_reference_row
        type: str
        size: 8
        encoding: BCS-N
        doc: Pixel Reference Row

      - id: pixel_reference_column
        type: str
        size: 8
        encoding: BCS-N
        doc: Pixel Reference Column

      - id: pixel_reference_value
        type: str
        size: 12
        encoding: BCS-N
        doc: Pixel Reference Value

  uncertainty_set_t:
    seq:
      - id: uncertainty_first_type
        type: str
        size: 11
        encoding: BCS-A
        doc: First Uncertainty Type

      - id: uncertainty_second_type
        type: str
        size: 11
        encoding: BCS-A
        doc: Second Uncertainty Type

      - id: uncertainty_value
        type: str
        size: 10
        encoding: BCS-N
        doc: Uncertainty Value

  additional_parameter_t:
    seq:
      - id: parameter_name
        type: str
        size: 25
        encoding: BCS-A
        doc: Additional Parameter Name

      - id: parameter_size
        type: str
        size: 3
        encoding: BCS-NPI
        doc: Additional Parameter Size (001-255)

      - id: parameter_count
        type: str
        size: 4
        encoding: BCS-NPI
        doc: Number of Parameter Values (0001-9999)

      - id: parameter_values
        type: str
        size: parameter_size.to_i
        encoding: BCS-A
        repeat: expr
        repeat-expr: parameter_count.to_i
