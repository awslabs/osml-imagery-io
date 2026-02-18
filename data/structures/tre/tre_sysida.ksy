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
  - id: platform_id_len
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Length of PLATFORM_ID field in bytes.
      3 BCS-N characters, range 000-999.

  - id: platform_id
    type: str
    size: platform_id_len.to_i
    encoding: ECS-A
    if: platform_id_len.to_i > 0
    doc: |
      Standard identifier for the collection platform.
      Value is case sensitive. See NITF Field Value Registry.
      Variable length ECS-A characters (length specified by PLATFORM_ID_LEN).

  - id: payload_id_len
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Length of PAYLOAD_ID field in bytes.
      3 BCS-N characters, range 000-999.

  - id: payload_id
    type: str
    size: payload_id_len.to_i
    encoding: ECS-A
    if: payload_id_len.to_i > 0
    doc: |
      Standard identifier for the payload.
      Value is case sensitive. See NITF Field Value Registry.
      Variable length ECS-A characters (length specified by PAYLOAD_ID_LEN).

  - id: sensor_id_len
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Length of SENSOR_ID field in bytes.
      3 BCS-N characters, range 000-999.

  - id: sensor_id
    type: str
    size: sensor_id_len.to_i
    encoding: ECS-A
    if: sensor_id_len.to_i > 0
    doc: |
      Standard identifier for the sensor.
      Value is case sensitive. See NITF Field Value Registry.
      Variable length ECS-A characters (length specified by SENSOR_ID_LEN).
