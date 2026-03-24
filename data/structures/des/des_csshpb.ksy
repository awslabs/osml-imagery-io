meta:
  id: des_csshpb
  title: CSSHPB DES User-Defined Subheader
  endian: be

doc: |
  CSSHPB DES - Coordinate System Shapefile Data Extension Segment (Version B)
  
  Extends CSSHPA to allow for significantly larger shapefiles, explicit tagging
  of the coordinate system, generalization beyond cloud shapes, and additional
  metadata to tie the shapefile to images within the NITF file.
  
  Version 1 (DESVER=01): Primary component files only, supporting files in separate DES
  Version 2 (DESVER=02): Supports embedding supporting component files in same DES
  
  The DESSHL for CSSHPB ranges from 0222 to 9999 bytes depending on the number
  of repeating fields (NUMAIS, NUM_ASSOC_ELEM, NUM_SHAPE_USE_ATTR, NUM_SUPPORTING_FILES).
  
  Note: This definition covers the DES-specific subheader fields (DESSHF)
  that appear when DESID is "CSSHPB DES" or "CSSHPB". The DESDATA field contains
  the concatenated shapefile component files.
  
  Reference: STDI-0002 Volume 2, Appendix D, Table D.5-1

seq:
  - id: SHAPEFILE_ID
    type: str
    size: 36
    encoding: BCS-A
    doc: |
      Shapefile ID (SHAPEFILE_ID)
      UUID in canonical form identifying the shapefile contained (or partially
      contained) in this DES. If the shapefile is split into multiple DES
      instances, all have the same SHAPEFILE_ID.
      36 BCS-A characters.

  - id: SHAPES_ID
    type: str
    size: 36
    encoding: BCS-A
    doc: |
      Shapes ID (SHAPES_ID)
      UUID in canonical form identifying the complete set of shapes across all
      shapefiles where the set was split into multiple independent shapefiles.
      36 BCS-A characters.

  - id: NUMAIS
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Number of Associated Image Segments (NUMAIS)
      Number of image segments associated with this Shapefile DES.
      3 BCS-A characters.
      Values: ALL (associated with all image segments), or 000-998

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
      with this DES. Repeated NUMAIS times.
      3 BCS-N characters.
      Range: 001 to 999
      Omitted if NUMAIS = "ALL" or "000".

  - id: TIMESTAMP
    type: str
    size: 24
    encoding: BCS-A
    doc: |
      UTC Timestamp (TIMESTAMP)
      UTC time at which this shapefile is associated.
      24 BCS-A characters in format CCYYMMDDhhmmss.sssssssss.
      Trailing digits set to hyphens if unknown precision.
      All spaces if not associated with a specific time.

  - id: NUM_ASSOC_ELEM
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Number of Associated Elements (NUM_ASSOC_ELEM)
      Number of elements associated with this shapefile.
      3 BCS-N characters.
      Range: 000 to 999

  - id: ASSOC_ELEM_ID
    type: str
    size: 36
    encoding: BCS-A
    repeat: expr
    repeat-expr: NUM_ASSOC_ELEM.to_i
    if: NUM_ASSOC_ELEM.to_i > 0
    doc: |
      Associated Element UUID (ASSOC_ELEM_IDi)
      UUID of the ith element associated with this shapefile.
      36 BCS-A characters in canonical UUID form.
      Repeated NUM_ASSOC_ELEM times.
      Omitted if NUM_ASSOC_ELEM = "000".

  - id: SHAPE_USE
    type: str
    size: 25
    encoding: BCS-A
    doc: |
      Shapefile Use (SHAPE_USE)
      Specifies the type of shapes contained within this DES.
      25 BCS-A characters.
      Values: IMAGE_SHAPE, CLOUD_SHAPES, MOSAIC_SOURCE_SHAPES, USER_DEF_SHAPES

  - id: NUM_SHAPE_USE_ATTR
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Number of Shape Use Attributes (NUM_SHAPE_USE_ATTR)
      Number of attributes to differentiate multiple instances with same SHAPE_USE.
      3 BCS-A characters.
      Range: 000 to 999

  - id: SHAPE_USE_ATTR
    type: shape_use_attr_record
    repeat: expr
    repeat-expr: NUM_SHAPE_USE_ATTR.to_i
    if: NUM_SHAPE_USE_ATTR.to_i > 0
    doc: |
      Shape Use Attribute records.
      Each record contains SHAPE_USE_ATTR_NAMEi (15 BCS-A) and
      SHAPE_USE_ATTR_VALi (10 BCS-A).
      Repeated NUM_SHAPE_USE_ATTR times.
      Omitted if NUM_SHAPE_USE_ATTR = "000".

  - id: SHAPE_CLASS
    type: str
    size: 11
    encoding: BCS-A
    doc: |
      Type of Shapes (SHAPE_CLASS)
      Type of shapes contained within this shapefile.
      11 BCS-A characters.
      Values: NULL SHAPE, POINT, POLYLINE, POLYGON, MULTIPOINT, POINTZ,
      POLYLINEZ, POLYGONZ, MULTIPOINTZ, POINTM, POLYLINEM, POLYGONM,
      MULTIPOINTM, MULTIPATCH

  - id: SHAPE_COORD
    type: str
    size: 10
    encoding: BCS-A
    doc: |
      Coordinate System (SHAPE_COORD)
      Coordinate system in which the shapes are specified.
      10 BCS-A characters.
      Values: WGS-84, LineSample

  - id: SHAPE_VERSION
    type: str
    size: 11
    encoding: BCS-N
    doc: |
      Shapefile Version (SHAPE_VERSION)
      Version number of the ESRI Shapefile Technical Description.
      11 BCS-N characters (signed, format %+011d).
      Range: -9999999999 to +9999999999

  - id: SHAPE_PART
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Shape Part Number (SHAPE_PART)
      Sequence number of the shapefile contained or partially contained in this DES.
      3 BCS-N characters.
      Range: 001 to 999

  - id: SHAPE_NUM_PARTS
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Total Number of Shape Parts (SHAPE_NUM_PARTS)
      Number of complete shapefiles necessary to store all shape data for a
      single SHAPE_USE and single set of attributes.
      3 BCS-N characters.
      Range: 001 to 999

  - id: SOURCE
    type: str
    size: 18
    encoding: BCS-A
    doc: |
      Source Sensor (SOURCE)
      Source sensor(s) from which the shape data was determined or generated.
      18 BCS-A characters.
      Values: PAN, MS, SWIR, CAVIS (or comma-separated combination)

  - id: SHAPE1_NAME
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Name of First Component File (SHAPE1_NAME)
      Name of first component file in the shapefile stored in this DES.
      3 BCS-A characters.
      Values: SHP, SHX, DBF (or spaces if no primary files in this DES for v2)

  - id: SHAPE1_START
    type: str
    size: 9
    encoding: BCS-A
    doc: |
      Offset to First Component File (SHAPE1_START)
      Start location in bytes of the first component file from the start of
      DES user-defined data.
      9 BCS-A characters.
      Range: 000000000 to 999999998 (or spaces if no primary files)

  - id: SHAPE2_NAME
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Name of Second Component File (SHAPE2_NAME)
      Name of second component file in the shapefile stored in this DES.
      3 BCS-A characters.
      Values: SHP, SHX, DBF (or spaces if only one file)

  - id: SHAPE2_START
    type: str
    size: 9
    encoding: BCS-A
    doc: |
      Offset to Second Component File (SHAPE2_START)
      Start location in bytes of the second component file.
      9 BCS-A characters.
      Range: 000000000 to 999999998 (or spaces if only one file)

  - id: SHAPE3_NAME
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Name of Third Component File (SHAPE3_NAME)
      Name of third component file in the shapefile stored in this DES.
      3 BCS-A characters.
      Values: SHP, SHX, DBF (or spaces if only one or two files)

  - id: SHAPE3_START
    type: str
    size: 9
    encoding: BCS-A
    doc: |
      Offset to Third Component File (SHAPE3_START)
      Start location in bytes of the third component file.
      9 BCS-A characters.
      Range: 000000000 to 999999998 (or spaces if only one or two files)

  # Version 2 fields - omitted if DESVER is 01
  # Note: The parser must check DESVER from the DES subheader to determine
  # presence. These fields are included unconditionally here since the KSY
  # definition covers the user-defined subheader only and DESVER is not
  # available in this scope. Callers must handle version gating externally.
  # When DESVER=01, parsing stops after SHAPE3_START (the remaining bytes
  # in the subheader will be zero).

  - id: REMAINING_DATA
    size-eos: true
    doc: |
      Remaining subheader data.
      For DESVER=02, this contains:
        NUM_SUPPORTING_FILES (2 BCS-N, range 00-99)
        For n = 1 to NUM_SUPPORTING_FILES:
          SUPPORTING_NAME_LENn (2 BCS-N, range 01-99)
          SUPPORTING_NAMEn (variable, SUPPORTING_NAME_LENn bytes BCS-A)
          SUPPORTING_STARTn (9 BCS-N, range 000000000-999999998)
          SUPPORTING_SIZEn (9 BCS-N, range 000000001-999999998)
      For DESVER=01, this field is empty (zero bytes).

types:
  shape_use_attr_record:
    seq:
      - id: SHAPE_USE_ATTR_NAME
        type: str
        size: 15
        encoding: BCS-A
        doc: |
          Shape Use Attribute Name (SHAPE_USE_ATTR_NAMEi)
          Name of the ith SHAPE_USE attribute.
          15 BCS-A characters. Values are case-insensitive.
          Values defined in Table D.5-3.

      - id: SHAPE_USE_ATTR_VAL
        type: str
        size: 10
        encoding: BCS-A
        doc: |
          Shape Use Attribute Value (SHAPE_USE_ATTR_VALi)
          Value of the ith SHAPE_USE attribute for this instance.
          10 BCS-A characters.
          Values defined independently for each attribute in Table D.5-3.
