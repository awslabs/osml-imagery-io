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
  
  Reference: STDI-0002 Volume 2, Appendix L, Table L.6-8

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
      Omitted if NUM_SHAPEFILES = "000" or "ALL".

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
    if: NUM_ASSOC_ELEM.to_i > 0
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
      Approved values include: 557_WW, AFWA, AUS_BOM, CAN_ENVIRON,
      EUMETCast, IHO, JMCC, NZL_METSERVICE, MOSC, NATO_METOC,
      NONTRADITIONAL, NWS, UK_MET_OFFICE, USGS, USNO, WMO,
      and others registered with NTB.

  # Conditional fields for METOC_SOURCE = NONTRADITIONAL
  - id: METOC_SOURCE_FORCE
    type: str
    size: 40
    encoding: BCS-A
    if: METOC_SOURCE.to_s.strip == "NONTRADITIONAL"
    doc: |
      Non-Traditional METOC Source Force (METOC_SOURCE_FORCE)
      The branch of the U.S. Armed Forces that acquired the
      non-traditional source of METOC data.
      40 BCS-A characters.
      Values: US_AIR_FORCE, US_ARMY, US_NAVY, US_SOCOM,
      US_COAST_GUARD, US_MARINE_CORPS, US_SPACE_FORCE,
      or all BCS spaces (0x20).
      Only present if METOC_SOURCE = "NONTRADITIONAL".

  - id: METOC_SOURCE_FORCE_UNIT
    type: str
    size: 240
    encoding: BCS-A
    if: METOC_SOURCE.to_s.strip == "NONTRADITIONAL"
    doc: |
      Non-Traditional METOC Source Unit (METOC_SOURCE_FORCE_UNIT)
      Free text string identifying the unit-level source of the
      non-traditional METOC data.
      240 BCS-A characters.
      Only present if METOC_SOURCE = "NONTRADITIONAL".

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
  - id: NUMPTS
    type: str
    size: 2
    encoding: BCS-N
    if: LOCATION_SHAPE.to_s.strip != ""
    doc: |
      Number of Points (NUMPTS)
      The number of points required to provide the location information.
      2 BCS-N characters.
      Values: 01 (POINT), 02 (LINE), 04-99 (POLYGON), 08 (VOLUME).
      Only present if LOCATION_SHAPE is not all spaces.

  - id: LOC_ELEV_REF
    type: str
    size: 3
    encoding: BCS-A
    if: LOCATION_SHAPE.to_s.strip != ""
    doc: |
      Location Elevation Reference (LOC_ELEV_REF)
      The vertical reference from which elevation information for the
      LOCATION_POINT_Zn field is reported.
      3 BCS-A characters.
      Values: HAE (WGS 84 ellipsoid), AGL (Above Ground Level),
      MSL (Mean Sea Level).
      Only present if LOCATION_SHAPE is not all spaces.

  - id: LOCATION_POINTS
    type: location_point_record
    repeat: expr
    repeat-expr: NUMPTS.to_i
    if: LOCATION_SHAPE.to_s.strip != ""
    doc: |
      Location point records. Each record contains longitude (14 BCS-N),
      latitude (13 BCS-N), and height (12 BCS-A) fields.
      Repeated NUMPTS times.
      Only present if LOCATION_SHAPE is not all spaces.

types:
  location_point_record:
    seq:
      - id: LOCATION_POINT_X
        type: str
        size: 14
        encoding: BCS-N
        doc: |
          Longitude of the Point (LOCATION_POINT_Xn)
          Longitude of the nth geographic point.
          14 BCS-N characters.
          Range: +/-180.000000000 degrees.
          Positive = east, negative = west of Prime Meridian.

      - id: LOCATION_POINT_Y
        type: str
        size: 13
        encoding: BCS-N
        doc: |
          Latitude of the Point (LOCATION_POINT_Yn)
          Latitude of the nth geographic point.
          13 BCS-N characters.
          Range: +/-90.000000000 degrees.
          Positive = north, negative = south of Equator.

      - id: LOCATION_POINT_Z
        type: str
        size: 12
        encoding: BCS-A
        doc: |
          Height Above Vertical Datum (LOCATION_POINT_Zn)
          Height with respect to the vertical reference defined in
          LOC_ELEV_REF, associated with the nth geographic point.
          12 BCS-A characters.
          Range: +/-9999999.999 meters, or BCS spaces if unknown.
