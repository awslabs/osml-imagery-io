meta:
  id: des_weather_data
  title: WEATHER_DATA DES User-Defined Subheader (DESVER=04)
  endian: be

doc: |
  WEATHER_DATA DES - Weather Data Extension Segment (Version 04)
  
  The WEATHER_DATA DES provides a means to encode meteorological and
  oceanographic (METOC) data, including weather data, within a NITF 2.1
  dataset. When a sensor remotely detects a signal, the intervening space
  between the sensor and the source of that signal may affect the quality
  of the signal received by the sensor, and the WEATHER_DATA DES
  characterizes the properties of that intervening space.
  
  This definition covers DESVER=04, which supports encapsulation of METOC
  datasets in their native formats including PAIS 2.0, GRIB0, GRIB1, GRIB2,
  and GRIB3.
  
  Note: DESVER=01 has DESSHL=0000 (no user-defined subheader fields).
  DESVER=04 has DESSHL range of 1410 to 9798 bytes.
  
  This definition covers the DES-specific subheader fields (DESSHF)
  that appear when DESID is "WEATHER_DATA" and DESVER is "04".
  The DESDATA field contains the METOC dataset in its native format.
  
  Reference: STDI-0002 Volume 2, Appendix L - WEATHER_DATA

seq:
  # DES specification and creation information
  - id: DES_DATE_TIME
    type: str
    size: 14
    encoding: ECS-A
    doc: |
      Date and Time of DES Creation (DES_DATE_TIME)
      The time of the WEATHER_DATA DES's creation.
      14 ECS-A characters in format CCYYMMDDhhmmss (UTC Zulu).
      May contain hyphen-minus (0x2D) for unknown portions.

  - id: DES_WRITER_NAME
    type: str
    size: 256
    encoding: ECS-A
    doc: |
      DES Writer Name (DES_WRITER_NAME)
      The name of the software used to create this instance of the
      WEATHER_DATA DES.
      256 ECS-A characters, alphanumeric or all ECS spaces (0x20).

  - id: DES_WRITER_VERSION
    type: str
    size: 66
    encoding: ECS-A
    doc: |
      DES Writer Version (DES_WRITER_VERSION)
      The version of the software used to create this instance of the
      WEATHER_DATA DES.
      66 ECS-A characters, alphanumeric or all ECS spaces (0x20).

  - id: DES_UUID
    type: str
    size: 36
    encoding: BCS-A
    doc: |
      UUID Assigned to this DES (DES_UUID)
      This UUID refers to the entire instance of this DES, i.e., both
      the subheader and data portions of the DES.
      36 BCS-A characters in canonical UUID format.
      Example: dbe26dc7-e003-4d29-8edb-41acc0e86b6e

  - id: NUMAIS
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Number of Associated Image Segments (NUMAIS)
      The number of image segments associated with this DES.
      3 BCS-A characters.
      Values: "ALL" or "000" to "998"
      If NUMAIS = "ALL", the DES is associated with all image segments
      in the NITF dataset, and field AISDLVLn is omitted.

  - id: AISDLVL
    type: str
    size: 3
    encoding: BCS-N
    repeat: expr
    repeat-expr: NUMAIS.to_i
    if: NUMAIS != "ALL" and NUMAIS != "000"
    doc: |
      Associated Image Segment Display Level (AISDLVLn)
      The Image Display Level (IDLVL) of each image segment associated
      with this DES.
      3 BCS-N characters.
      Range: 001 to 999
      Omitted if NUMAIS = "000" or "ALL".

  - id: NUM_SHAPEFILES
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Number of Associated Shapefile DESs (NUM_SHAPEFILES)
      The number of shapefile DESs associated with this DES.
      3 BCS-A characters.
      Values: "ALL" or "000" to "999"
      If NUM_SHAPEFILES = "ALL", the DES is associated with all
      shapefile DESs in the NITF dataset.

  - id: SHAPEFILE_UUID
    type: str
    size: 36
    encoding: BCS-N
    repeat: expr
    repeat-expr: NUM_SHAPEFILES.to_i
    if: NUM_SHAPEFILES != "ALL" and NUM_SHAPEFILES != "000"
    doc: |
      UUID of Associated Shapefile DES (SHAPEFILE_UUIDn)
      The UUID value of each shapefile DES associated with this DES.
      36 BCS-N characters in canonical UUID format.
      Omitted if NUM_SHAPEFILES = "0" or "ALL".

  - id: NUM_ASSOC_ELEM
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Number of Associated Elements (NUM_ASSOC_ELEM)
      The number of elements associated with this DES, besides any
      shapefile DESs, that have assigned UUIDs.
      3 BCS-A characters.
      Range: 000 to 999

  - id: ASSOC_ELEM_UUID
    type: str
    size: 36
    encoding: BCS-N
    repeat: expr
    repeat-expr: NUM_ASSOC_ELEM.to_i
    doc: |
      UUID of Associated Element (ASSOC_ELEM_UUIDn)
      The UUID of the nth element associated with this DES.
      36 BCS-N characters in canonical UUID format.
      Omitted if NUM_ASSOC_ELEM = "000".

  # METOC creation and content information
  - id: METOC_WRITER_NAME
    type: str
    size: 256
    encoding: ECS-A
    doc: |
      METOC Writer Name (METOC_WRITER_NAME)
      The name of the software used to create the METOC dataset stored
      in the user-defined data (DESDATA) portion of this DES.
      256 ECS-A characters, alphanumeric or all ECS spaces (0x20).

  - id: METOC_WRITER_VERSION
    type: str
    size: 66
    encoding: ECS-A
    doc: |
      METOC Writer Version (METOC_WRITER_VERSION)
      The version of the software used to create the METOC dataset.
      66 ECS-A characters, alphanumeric or all ECS spaces (0x20).

  - id: ATMOS_FLAG
    type: str
    size: 1
    encoding: ECS-A
    doc: |
      Atmospheric Data Flag (ATMOS_FLAG)
      Indicates whether the METOC dataset contains information about
      the atmospheric environment.
      1 ECS-A character.
      Values: Y = Data included, N = Data not included, U = Unknown

  - id: OCEAN_FLAG
    type: str
    size: 1
    encoding: ECS-A
    doc: |
      Oceanographic Data Flag (OCEAN_FLAG)
      Indicates whether the METOC dataset contains information about
      the marine environment.
      1 ECS-A character.
      Values: Y = Data included, N = Data not included, U = Unknown

  - id: SPACE_FLAG
    type: str
    size: 1
    encoding: ECS-A
    doc: |
      Space Data Flag (SPACE_FLAG)
      Indicates whether the METOC dataset contains information about
      the space environment.
      1 ECS-A character.
      Values: Y = Data included, N = Data not included, U = Unknown

  - id: METOC_SOURCE
    type: str
    size: 80
    encoding: ECS-A
    doc: |
      Source of the METOC Dataset (METOC_SOURCE)
      The name of the organization that created the METOC information.
      80 ECS-A characters.
      Approved values include: AFWA, FNMOC, NCEP, ECMWF, UKMO, JMA,
      EUMETSAT, NONTRADITIONAL, and others registered with NTB.

  # Conditional fields for METOC_SOURCE = NONTRADITIONAL
  # Note: These fields are only present if METOC_SOURCE = "NONTRADITIONAL"
  # The parser must check METOC_SOURCE value to determine presence

  - id: METOC_FORMAT
    type: str
    size: 80
    encoding: ECS-A
    doc: |
      Format of the METOC Dataset (METOC_FORMAT)
      The name and version of the METOC dataset stored in DESDATA.
      80 ECS-A characters.
      Approved values: PAIS_2.0, GRIB0, GRIB1, GRIB2, GRIB3

  - id: COM_SIZE
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Size in Bytes of the Comment Block (COM_SIZE)
      The number of bytes used to record the block of free text.
      4 BCS-N characters.
      Range: 0000 to 8388

  - id: COMMENTS
    type: str
    size: COM_SIZE.to_i
    encoding: ECS-A
    if: COM_SIZE.to_i > 0
    doc: |
      Free Text Comment Block (COMMENTS)
      A block of user-defined free text.
      Size determined by COM_SIZE field value.
      Omitted if COM_SIZE is "0000".

  - id: CREATION_TIMESTAMP
    type: str
    size: 14
    encoding: ECS-A
    doc: |
      Creation Timestamp of the METOC Dataset (CREATION_TIMESTAMP)
      The timestamp associated with the creation of the METOC dataset.
      14 ECS-A characters in format CCYYMMDDhhmmss (UTC Zulu).
      May be all ECS spaces (0x20) if unknown.

  - id: VALID_TIMESTAMP
    type: str
    size: 14
    encoding: ECS-A
    doc: |
      Validity Timestamp of the METOC Dataset (VALID_TIMESTAMP)
      The timestamp for when the METOC data are valid or applicable.
      14 ECS-A characters in format CCYYMMDDhhmmss (UTC Zulu).
      May be all ECS spaces (0x20) if unknown.

  - id: METOC_GENERATION
    type: str
    size: 80
    encoding: ECS-A
    doc: |
      Generation or Type of the METOC Dataset (METOC_GENERATION)
      Indicates whether the METOC dataset was generated using observed
      or prognostic model data, or some combination.
      80 ECS-A characters.
      Values depend on METOC_FORMAT (see Tables L.6-3 through L.6-6).

  - id: LOCATION_SHAPE
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      Location Shape (LOCATION_SHAPE)
      Indicates the shape of the location information applicable to
      the METOC dataset, if any.
      40 BCS-A characters.
      Values: POINT, LINE, POLYGON, VOLUME, or BCS spaces (0x20).

  # Conditional location fields - only present if LOCATION_SHAPE != spaces
  # Note: The parser must check LOCATION_SHAPE to determine presence
