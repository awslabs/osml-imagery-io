meta:
  id: tre_sensrb
  title: General Electro-Optical Sensor Parameters TRE
  endian: be

doc: |
  SENSRB TRE - General Electro-Optical Sensor Parameters
  Version 2.2
  
  Provides sensor parameters for imaging electro-optical (EO) sensors including
  visible, infrared, multi- and hyperspectral sensors. Contains 15 conditional
  modules for SENSOR identification, array parameters, calibration, image formation,
  position, attitude, velocity, point sets, time-stamped data, pixel-referenced data,
  uncertainty data, and additional parameters.
  
  Reference: STDI-0002 Volume 1, Appendix Z - SENSRB

seq:
  # Module 01: General Data
  - id: GENERAL_DATA
    type: str
    size: 1
    encoding: BCS-A
    doc: General Data Flag (Y/N)

  - id: GENERAL_DATA_MODULE
    type: general_data_module_t
    if: GENERAL_DATA == "Y"

  # Module 02: Sensor Array Data
  - id: SENSOR_ARRAY_DATA
    type: str
    size: 1
    encoding: BCS-A
    doc: Sensor Array Data Flag (Y/N)

  - id: SENSOR_ARRAY_MODULE
    type: sensor_array_module_t
    if: SENSOR_ARRAY_DATA == "Y"

  # Module 03: Sensor Calibration Data
  - id: SENSOR_CALIBRATION_DATA
    type: str
    size: 1
    encoding: BCS-A
    doc: Sensor Calibration Data Flag (Y/N)

  - id: SENSOR_CALIBRATION_MODULE
    type: sensor_calibration_module_t
    if: SENSOR_CALIBRATION_DATA == "Y"

  # Module 04: Image Formation Data
  - id: IMAGE_FORMATION_DATA
    type: str
    size: 1
    encoding: BCS-A
    doc: Image Formation Data Flag (Y/N)

  - id: IMAGE_FORMATION_MODULE
    type: image_formation_module_t
    if: IMAGE_FORMATION_DATA == "Y"

  # Module 05: Reference Time/Pixel
  - id: REFERENCE_TIME
    type: str
    size: 12
    encoding: BCS-N
    doc: Reference Time of Applicability (seconds relative to START_TIME)

  - id: REFERENCE_ROW
    type: str
    size: 8
    encoding: BCS-N
    doc: Reference Pixel Row of Applicability

  - id: REFERENCE_COLUMN
    type: str
    size: 8
    encoding: BCS-N
    doc: Reference Pixel Column of Applicability

  # Module 06: Sensor Position Data (Required)
  - id: LATITUDE_OR_X
    type: str
    size: 11
    encoding: BCS-N
    doc: Sensor/Platform Latitude or ECEF X Position

  - id: LONGITUDE_OR_Y
    type: str
    size: 12
    encoding: BCS-N
    doc: Sensor/Platform Longitude or ECEF Y Position

  - id: ALTITUDE_OR_Z
    type: str
    size: 11
    encoding: BCS-N
    doc: Sensor/Platform Altitude or ECEF Z Position

  - id: SENSOR_X_OFFSET
    type: str
    size: 8
    encoding: BCS-N
    doc: Sensor X Position Offset Relative to Platform

  - id: SENSOR_Y_OFFSET
    type: str
    size: 8
    encoding: BCS-N
    doc: Sensor Y Position Offset Relative to Platform

  - id: SENSOR_Z_OFFSET
    type: str
    size: 8
    encoding: BCS-N
    doc: Sensor Z Position Offset Relative to Platform

  # Module 07: Attitude Euler Angles
  - id: ATTITUDE_EULER_ANGLES
    type: str
    size: 1
    encoding: BCS-A
    doc: Attitude Euler Angles Flag (Y/N)

  - id: ATTITUDE_EULER_MODULE
    type: attitude_euler_module_t
    if: ATTITUDE_EULER_ANGLES == "Y"

  # Module 08: Attitude Unit Vectors
  - id: ATTITUDE_UNIT_VECTORS
    type: str
    size: 1
    encoding: BCS-A
    doc: Attitude Unit Vectors Flag (Y/N)

  - id: ATTITUDE_UNIT_VECTORS_MODULE
    type: attitude_unit_vectors_module_t
    if: ATTITUDE_UNIT_VECTORS == "Y"

  # Module 09: Attitude Quaternion
  - id: ATTITUDE_QUATERNION
    type: str
    size: 1
    encoding: BCS-A
    doc: Attitude Quaternion Flag (Y/N)

  - id: ATTITUDE_QUATERNION_MODULE
    type: attitude_quaternion_module_t
    if: ATTITUDE_QUATERNION == "Y"

  # Module 10: Sensor Velocity Data
  - id: SENSOR_VELOCITY_DATA
    type: str
    size: 1
    encoding: BCS-A
    doc: Sensor Velocity Data Flag (Y/N)

  - id: SENSOR_VELOCITY_MODULE
    type: sensor_velocity_module_t
    if: SENSOR_VELOCITY_DATA == "Y"

  # Module 11: Point Set Data
  - id: POINT_SET_DATA
    type: str
    size: 2
    encoding: BCS-NPI
    doc: Number of Point Sets (00-99)

  - id: POINT_SETS
    type: point_set_t
    repeat: expr
    repeat-expr: POINT_SET_DATA.to_i
    if: POINT_SET_DATA.to_i > 0

  # Module 12: Time Stamped Data Sets
  - id: TIME_STAMPED_DATA_SETS
    type: str
    size: 2
    encoding: BCS-NPI
    doc: Number of Time Stamped Data Sets (00-99)

  - id: TIME_STAMPED_SETS
    type: time_stamped_set_t
    repeat: expr
    repeat-expr: TIME_STAMPED_DATA_SETS.to_i
    if: TIME_STAMPED_DATA_SETS.to_i > 0

  # Module 13: Pixel Referenced Data Sets
  - id: PIXEL_REFERENCED_DATA_SETS
    type: str
    size: 2
    encoding: BCS-NPI
    doc: Number of Pixel Referenced Data Sets (00-99)

  - id: PIXEL_REFERENCED_SETS
    type: pixel_referenced_set_t
    repeat: expr
    repeat-expr: PIXEL_REFERENCED_DATA_SETS.to_i
    if: PIXEL_REFERENCED_DATA_SETS.to_i > 0

  # Module 14: Uncertainty Data
  - id: UNCERTAINTY_DATA
    type: str
    size: 3
    encoding: BCS-NPI
    doc: Number of Uncertainty Data Sets (000-999)

  - id: UNCERTAINTY_SETS
    type: uncertainty_set_t
    repeat: expr
    repeat-expr: UNCERTAINTY_DATA.to_i
    if: UNCERTAINTY_DATA.to_i > 0

  # Module 15: Additional Parameter Data
  - id: ADDITIONAL_PARAMETER_DATA
    type: str
    size: 3
    encoding: BCS-NPI
    doc: Number of Additional Parameters (000-999)

  - id: ADDITIONAL_PARAMETERS
    type: additional_parameter_t
    repeat: expr
    repeat-expr: ADDITIONAL_PARAMETER_DATA.to_i
    if: ADDITIONAL_PARAMETER_DATA.to_i > 0

types:
  general_data_module_t:
    seq:
      - id: SENSOR
        type: str
        size: 25
        encoding: BCS-A
        doc: Sensor Registered Name or Model

      - id: SENSOR_URI
        type: str
        size: 32
        encoding: BCS-A
        doc: Sensor Uniform Resource Identifier

      - id: PLATFORM
        type: str
        size: 25
        encoding: BCS-A
        doc: Platform Common Name

      - id: PLATFORM_URI
        type: str
        size: 32
        encoding: BCS-A
        doc: Platform Uniform Resource Identifier

      - id: OPERATION_DOMAIN
        type: str
        size: 10
        encoding: BCS-A
        doc: Operational Domain (Airborne, Spaceborne, Waterborne, Ground)

      - id: CONTENT_LEVEL
        type: str
        size: 1
        encoding: BCS-NPI
        doc: Content Level (0-9)

      - id: GEODETIC_SYSTEM
        type: str
        size: 5
        encoding: BCS-A
        doc: Geodetic Reference System (default WGS84)

      - id: GEODETIC_TYPE
        type: str
        size: 1
        encoding: BCS-A
        doc: Geodetic Coordinate Type (G=Geographic, C=Geocentric)

      - id: ELEVATION_DATUM
        type: str
        size: 3
        encoding: BCS-A
        doc: Elevation/Altitude Datum (HAE, MSL, AGL)

      - id: LENGTH_UNIT
        type: str
        size: 2
        encoding: BCS-A
        doc: Length Unit System (SI or EE)

      - id: ANGULAR_UNIT
        type: str
        size: 3
        encoding: BCS-A
        doc: Angular Unit Type (DEG, RAD, SMC)

      - id: START_DATE
        type: str
        size: 8
        encoding: BCS-NI
        doc: Imaging Start Date (YYYYMMDD)

      - id: START_TIME
        type: str
        size: 14
        encoding: BCS-N
        doc: Imaging Start Time (seconds into day)

      - id: END_DATE
        type: str
        size: 8
        encoding: BCS-NI
        doc: Imaging End Date (YYYYMMDD)

      - id: END_TIME
        type: str
        size: 14
        encoding: BCS-N
        doc: Imaging End Time (seconds into day)

      - id: GENERATION_COUNT
        type: str
        size: 2
        encoding: BCS-NPI
        doc: Generation Count (00-99)

      - id: GENERATION_DATE
        type: str
        size: 8
        encoding: BCS-NI
        doc: Generation Date (YYYYMMDD)

      - id: GENERATION_TIME
        type: str
        size: 10
        encoding: BCS-N
        doc: Generation Time (HHMMSS.sss)

  sensor_array_module_t:
    seq:
      - id: DETECTION
        type: str
        size: 20
        encoding: BCS-A
        doc: Detection Type

      - id: ROW_DETECTORS
        type: str
        size: 8
        encoding: BCS-NPI
        doc: Number of Detector Rows

      - id: COLUMN_DETECTORS
        type: str
        size: 8
        encoding: BCS-NPI
        doc: Number of Detector Columns

      - id: ROW_METRIC
        type: str
        size: 8
        encoding: BCS-N
        doc: Physical Dimension of Used Rows (cm or in)

      - id: COLUMN_METRIC
        type: str
        size: 8
        encoding: BCS-N
        doc: Physical Dimension of Used Columns (cm or in)

      - id: FOCAL_LENGTH
        type: str
        size: 8
        encoding: BCS-N
        doc: Best Known Focal Length (cm or in)

      - id: ROW_FOV
        type: str
        size: 8
        encoding: BCS-N
        doc: Field of View - Rows (deg, rad, or smc)

      - id: COLUMN_FOV
        type: str
        size: 8
        encoding: BCS-N
        doc: Field of View - Columns (deg, rad, or smc)

      - id: CALIBRATED
        type: str
        size: 1
        encoding: BCS-A
        doc: Focal Length Calibration Flag (Y/N)

  sensor_calibration_module_t:
    seq:
      - id: CALIBRATION_UNIT
        type: str
        size: 2
        encoding: BCS-A
        doc: Calibration Unit System (mm or px)

      - id: PRINCIPAL_POINT_OFFSET_X
        type: str
        size: 9
        encoding: BCS-N
        doc: Principal Point Offset X

      - id: PRINCIPAL_POINT_OFFSET_Y
        type: str
        size: 9
        encoding: BCS-N
        doc: Principal Point Offset Y

      - id: RADIAL_DISTORT_1
        type: str
        size: 12
        encoding: BCS-A
        doc: First Radial Distortion Coefficient (k1)

      - id: RADIAL_DISTORT_2
        type: str
        size: 12
        encoding: BCS-A
        doc: Second Radial Distortion Coefficient (k2)

      - id: RADIAL_DISTORT_3
        type: str
        size: 12
        encoding: BCS-A
        doc: Third Radial Distortion Coefficient (k3)

      - id: RADIAL_DISTORT_LIMIT
        type: str
        size: 9
        encoding: BCS-N
        doc: Limit of Radial Distortion Fit

      - id: DECENT_DISTORT_1
        type: str
        size: 12
        encoding: BCS-A
        doc: First Decentering Distortion Coefficient (p1)

      - id: DECENT_DISTORT_2
        type: str
        size: 12
        encoding: BCS-A
        doc: Second Decentering Distortion Coefficient (p2)

      - id: AFFINITY_DISTORT_1
        type: str
        size: 12
        encoding: BCS-A
        doc: First Affinity Distortion Coefficient (b1)

      - id: AFFINITY_DISTORT_2
        type: str
        size: 12
        encoding: BCS-A
        doc: Second Affinity Distortion Coefficient (b2)

      - id: CALIBRATION_DATE
        type: str
        size: 8
        encoding: BCS-NI
        doc: Calibration Report Date (YYYYMMDD)


  image_formation_module_t:
    seq:
      - id: METHOD
        type: str
        size: 15
        encoding: BCS-A
        doc: Image Formation Method (Single Frame, Continuous, etc.)

      - id: MODE
        type: str
        size: 3
        encoding: BCS-A
        doc: Imaging Mode (PAN, MS, HS, etc.)

      - id: ROW_COUNT
        type: str
        size: 8
        encoding: BCS-NPI
        doc: Number of Image Rows

      - id: COLUMN_COUNT
        type: str
        size: 8
        encoding: BCS-NPI
        doc: Number of Image Columns

      - id: ROW_SET
        type: str
        size: 8
        encoding: BCS-NPI
        doc: Row Set (first row of image in detector array)

      - id: COLUMN_SET
        type: str
        size: 8
        encoding: BCS-NPI
        doc: Column Set (first column of image in detector array)

      - id: ROW_RATE
        type: str
        size: 10
        encoding: BCS-N
        doc: Row Rate (rows per second)

      - id: COLUMN_RATE
        type: str
        size: 10
        encoding: BCS-N
        doc: Column Rate (columns per second)

      - id: FIRST_PIXEL_ROW
        type: str
        size: 8
        encoding: BCS-NPI
        doc: First Pixel Row

      - id: FIRST_PIXEL_COLUMN
        type: str
        size: 8
        encoding: BCS-NPI
        doc: First Pixel Column

      - id: TRANSFORM_PARAMS
        type: str
        size: 1
        encoding: BCS-NPI
        doc: Number of Transform Parameters (0-6)

      - id: TRANSFORM_PARAM_VALUES
        type: str
        size: 12
        encoding: BCS-N
        repeat: expr
        repeat-expr: TRANSFORM_PARAMS.to_i
        if: TRANSFORM_PARAMS.to_i > 0

  attitude_euler_module_t:
    seq:
      - id: SENSOR_ANGLE_MODEL
        type: str
        size: 1
        encoding: BCS-NPI
        doc: Sensor Angle Model (1-4)

      - id: SENSOR_ANGLE_1
        type: str
        size: 10
        encoding: BCS-N
        doc: Sensor Angle 1 (deg, rad, or smc)

      - id: SENSOR_ANGLE_2
        type: str
        size: 9
        encoding: BCS-N
        doc: Sensor Angle 2 (deg, rad, or smc)

      - id: SENSOR_ANGLE_3
        type: str
        size: 10
        encoding: BCS-N
        doc: Sensor Angle 3 (deg, rad, or smc)

      - id: PLATFORM_RELATIVE
        type: str
        size: 1
        encoding: BCS-A
        doc: Platform Relative Flag (Y/N)

      - id: PLATFORM_HEADING
        type: str
        size: 9
        encoding: BCS-N
        doc: Platform Heading (deg, rad, or smc)

      - id: PLATFORM_PITCH
        type: str
        size: 9
        encoding: BCS-N
        doc: Platform Pitch (deg, rad, or smc)

      - id: PLATFORM_ROLL
        type: str
        size: 10
        encoding: BCS-N
        doc: Platform Roll (deg, rad, or smc)

  attitude_unit_vectors_module_t:
    seq:
      - id: ICX_NORTH_OR_X
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column X Unit Vector - North or X Component

      - id: ICX_EAST_OR_Y
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column X Unit Vector - East or Y Component

      - id: ICX_DOWN_OR_Z
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column X Unit Vector - Down or Z Component

      - id: ICY_NORTH_OR_X
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column Y Unit Vector - North or X Component

      - id: ICY_EAST_OR_Y
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column Y Unit Vector - East or Y Component

      - id: ICY_DOWN_OR_Z
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column Y Unit Vector - Down or Z Component

      - id: ICZ_NORTH_OR_X
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column Z Unit Vector - North or X Component

      - id: ICZ_EAST_OR_Y
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column Z Unit Vector - East or Y Component

      - id: ICZ_DOWN_OR_Z
        type: str
        size: 10
        encoding: BCS-N
        doc: Image Column Z Unit Vector - Down or Z Component

  attitude_quaternion_module_t:
    seq:
      - id: ATTITUDE_Q1
        type: str
        size: 10
        encoding: BCS-N
        doc: Attitude Quaternion Component Q1

      - id: ATTITUDE_Q2
        type: str
        size: 10
        encoding: BCS-N
        doc: Attitude Quaternion Component Q2

      - id: ATTITUDE_Q3
        type: str
        size: 10
        encoding: BCS-N
        doc: Attitude Quaternion Component Q3

      - id: ATTITUDE_Q4
        type: str
        size: 10
        encoding: BCS-N
        doc: Attitude Quaternion Component Q4

  sensor_velocity_module_t:
    seq:
      - id: VELOCITY_NORTH_OR_X
        type: str
        size: 9
        encoding: BCS-N
        doc: Sensor Velocity - North or X Component (m/s or ft/s)

      - id: VELOCITY_EAST_OR_Y
        type: str
        size: 9
        encoding: BCS-N
        doc: Sensor Velocity - East or Y Component (m/s or ft/s)

      - id: VELOCITY_DOWN_OR_Z
        type: str
        size: 9
        encoding: BCS-N
        doc: Sensor Velocity - Down or Z Component (m/s or ft/s)

  point_set_t:
    seq:
      - id: POINT_SET_TYPE
        type: str
        size: 25
        encoding: BCS-A
        doc: Point Set Type Description

      - id: POINT_COUNT
        type: str
        size: 3
        encoding: BCS-NPI
        doc: Number of Points in Set (001-999)

      - id: POINTS
        type: point_t
        repeat: expr
        repeat-expr: POINT_COUNT.to_i

  point_t:
    seq:
      - id: ROW
        type: str
        size: 8
        encoding: BCS-N
        doc: Point Row Location

      - id: COLUMN
        type: str
        size: 8
        encoding: BCS-N
        doc: Point Column Location

      - id: LATITUDE
        type: str
        size: 10
        encoding: BCS-N
        doc: Point Latitude

      - id: LONGITUDE
        type: str
        size: 11
        encoding: BCS-N
        doc: Point Longitude

      - id: ELEVATION
        type: str
        size: 6
        encoding: BCS-N
        doc: Point Elevation

      - id: RANGE
        type: str
        size: 8
        encoding: BCS-N
        doc: Point Range

  time_stamped_set_t:
    seq:
      - id: TIME_STAMP_TYPE
        type: str
        size: 3
        encoding: BCS-A
        doc: Time Stamp Type (06b, 06c, 07a, 07b, 07c, 08a, 08b, 08c, 09a, 10a)

      - id: TIME_STAMP_COUNT
        type: str
        size: 4
        encoding: BCS-NPI
        doc: Number of Time Stamps (0001-9999)

      - id: TIME_STAMPS
        type: time_stamp_t
        repeat: expr
        repeat-expr: TIME_STAMP_COUNT.to_i

  time_stamp_t:
    seq:
      - id: TIME_STAMP_TIME
        type: str
        size: 12
        encoding: BCS-N
        doc: Time Stamp Time (seconds relative to START_TIME)

      - id: TIME_STAMP_VALUE
        type: str
        size: 12
        encoding: BCS-N
        doc: Time Stamp Value

  pixel_referenced_set_t:
    seq:
      - id: PIXEL_REFERENCE_TYPE
        type: str
        size: 3
        encoding: BCS-A
        doc: Pixel Reference Type (06b, 06c, 07a, 07b, 07c, 08a, 08b, 08c, 09a, 10a)

      - id: PIXEL_REFERENCE_COUNT
        type: str
        size: 4
        encoding: BCS-NPI
        doc: Number of Pixel References (0001-9999)

      - id: PIXEL_REFERENCES
        type: pixel_reference_t
        repeat: expr
        repeat-expr: PIXEL_REFERENCE_COUNT.to_i

  pixel_reference_t:
    seq:
      - id: PIXEL_REFERENCE_ROW
        type: str
        size: 8
        encoding: BCS-N
        doc: Pixel Reference Row

      - id: PIXEL_REFERENCE_COLUMN
        type: str
        size: 8
        encoding: BCS-N
        doc: Pixel Reference Column

      - id: PIXEL_REFERENCE_VALUE
        type: str
        size: 12
        encoding: BCS-N
        doc: Pixel Reference Value

  uncertainty_set_t:
    seq:
      - id: UNCERTAINTY_FIRST_TYPE
        type: str
        size: 11
        encoding: BCS-A
        doc: First Uncertainty Type

      - id: UNCERTAINTY_SECOND_TYPE
        type: str
        size: 11
        encoding: BCS-A
        doc: Second Uncertainty Type

      - id: UNCERTAINTY_VALUE
        type: str
        size: 10
        encoding: BCS-N
        doc: Uncertainty Value

  additional_parameter_t:
    seq:
      - id: PARAMETER_NAME
        type: str
        size: 25
        encoding: BCS-A
        doc: Additional Parameter Name

      - id: PARAMETER_SIZE
        type: str
        size: 3
        encoding: BCS-NPI
        doc: Additional Parameter Size (001-255)

      - id: PARAMETER_COUNT
        type: str
        size: 4
        encoding: BCS-NPI
        doc: Number of Parameter Values (0001-9999)

      - id: PARAMETER_VALUES
        type: str
        size: PARAMETER_SIZE.to_i
        encoding: BCS-A
        repeat: expr
        repeat-expr: PARAMETER_COUNT.to_i
