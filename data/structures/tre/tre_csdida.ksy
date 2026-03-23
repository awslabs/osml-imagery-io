meta:
  id: tre_csdida
  title: Dataset Identification TRE
  endian: be

doc: |
  CSDIDA TRE - Dataset Identification
  
  Provides basic information describing the data contained in the NITF file
  including collection date, platform, sensor, and processing information.
  
  This TRE is required in the NITF file header of every commercial dataset.
  
  Reference: STDI-0006 (NCDRD), Table 3.3-1

seq:
  - id: DAY
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Day of Dataset Collection (UTC)
      Day of start of dataset collection (Image Start Time).
      Range: 01 to 31

  - id: MONTH
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Month of Dataset Collection (UTC)
      Month of start of dataset collection.
      Values: JAN to DEC

  - id: YEAR
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Year of Dataset Collection (UTC)
      Four-digit year of start of dataset collection.
      Range: 0000 to 9999

  - id: PLATFORM_CODE
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Platform Identification
      Source satellite platform code.
      Values: QB, IK, OV, WV

  - id: VEHICLE_ID
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Vehicle Number
      Vehicle number of the source satellite.
      Range: 00 to 99

  - id: PASS
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Pass Number
      Supplier-selected pass number.
      Range: 01 to 99

  - id: OPERATION
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Operation Number
      Supplier-selected operation number.
      Range: 001 to 999 (may be 000 if supplier does not use operation counts)

  - id: SENSOR_ID
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Sensor ID
      Identifies the type of payload data collection.
      AA = panchromatic only
      GA = multispectral and pan-sharpened only
      NA = panchromatic & multispectral together

  - id: PRODUCT_ID
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Product ID
      Identifies the broad class of commercial products.
      Refer to NCDRD Table 2.1-7 for Image Product Types.

  - id: RESERVED_1
    type: str
    size: 4
    encoding: BCS-A
    doc: |
      Reserved Fill
      Value: 0000

  - id: TIME
    type: str
    size: 14
    encoding: BCS-N
    doc: |
      Image Start Time (UTC)
      Same time as defined in NITF Image Segment Subheader IDATIM field.
      Format: YYYYMMDDhhmmss

  - id: PROCESS_TIME
    type: str
    size: 14
    encoding: BCS-N
    doc: |
      Process Completion Time (UTC)
      Time of NITF file creation. Same as NITF File Header FDT field.
      Format: YYYYMMDDhhmmss

  - id: RESERVED_2
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Reserved Fill
      Value: 00

  - id: RESERVED_3
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Reserved Fill
      Value: 01

  - id: RESERVED_4
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Reserved Fill
      Value: N

  - id: RESERVED_5
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Reserved Fill
      Value: N

  - id: SOFTWARE_VERSION_NUMBER
    type: str
    size: 10
    encoding: BCS-A
    doc: |
      Software Version Number
      Software version used for dataset processing.
      Vendor defined.
