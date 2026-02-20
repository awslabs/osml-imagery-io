meta:
  id: tre_sysida
  title: System Identification TRE
  endian: be

doc: |
  SYSIDA TRE - System Identification Tagged Record Extension
  
  Provides standard identifiers for the collection platform, payload, and sensor.
  This TRE allows identification of the collection system for all collected imagery
  and supplemental data associated with a specific collection system.
  
  The TRE has a variable length (9 to 3006 bytes) with three length-prefixed
  identifier fields:
  - PLATFORM_ID: Standard identifier for the collection platform
  - PAYLOAD_ID: Standard identifier for the payload
  - SENSOR_ID: Standard identifier for the sensor
  
  At least one of the identifier fields must be populated with a non-zero length value.
  
  Unlike CSEXRB, PIAIMC and ACFTB where fields are fixed six-character fields,
  the identifier values in SYSIDA are not padded with spaces.
  
  Reference: STDI-0002 Volume 1, Appendix AS - SYSIDA v1.0

seq:
  - id: PLATFORM_ID_LEN
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Length of PLATFORM_ID field in bytes.
      3 BCS-N characters, range 000-999.

  - id: PLATFORM_ID
    type: str
    size: PLATFORM_ID_LEN.to_i
    encoding: ECS-A
    if: PLATFORM_ID_LEN.to_i > 0
    doc: |
      Standard identifier for the collection platform.
      Value is case sensitive. See NITF Field Value Registry.
      Variable length ECS-A characters (length specified by PLATFORM_ID_LEN).

  - id: PAYLOAD_ID_LEN
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Length of PAYLOAD_ID field in bytes.
      3 BCS-N characters, range 000-999.

  - id: PAYLOAD_ID
    type: str
    size: PAYLOAD_ID_LEN.to_i
    encoding: ECS-A
    if: PAYLOAD_ID_LEN.to_i > 0
    doc: |
      Standard identifier for the payload.
      Value is case sensitive. See NITF Field Value Registry.
      Variable length ECS-A characters (length specified by PAYLOAD_ID_LEN).

  - id: SENSOR_ID_LEN
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Length of SENSOR_ID field in bytes.
      3 BCS-N characters, range 000-999.

  - id: SENSOR_ID
    type: str
    size: SENSOR_ID_LEN.to_i
    encoding: ECS-A
    if: SENSOR_ID_LEN.to_i > 0
    doc: |
      Standard identifier for the sensor.
      Value is case sensitive. See NITF Field Value Registry.
      Variable length ECS-A characters (length specified by SENSOR_ID_LEN).
