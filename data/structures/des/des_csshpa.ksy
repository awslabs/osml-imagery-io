meta:
  id: des_csshpa
  title: CSSHPA DES User-Defined Subheader
  endian: be

doc: |
  CSSHPA DES - Coordinate System Shapefile Data Extension Segment (Version A)
  
  Allows an ESRI shapefile to be embedded in a NITF file, along with additional
  metadata to identify and describe the purpose of the shapefile. The CSSHPA DES
  embeds the three primary component files (SHP, SHX, DBF) of a shapefile.
  
  The DESSHL for CSSHPA is either:
  - 0062 bytes (when SHAPE_USE is not CLOUD_SHAPES)
  - 0080 bytes (when SHAPE_USE is CLOUD_SHAPES, includes CC_SOURCE field)
  
  Note: This definition covers the DES-specific subheader fields (DESSHF)
  that appear when DESID is "CSSHPA DES". The DESDATA field contains the
  concatenated shapefile component files.
  
  Reference: STDI-0002 Volume 2, Appendix D - CSSHPA-CSSHPB
  Reference: STDI-0006 (for systems compliant with STDI-0006)

seq:
  - id: shape_use
    type: str
    size: 25
    encoding: BCS-A
    doc: |
      Shapefile Use (SHAPE_USE)
      Specifies the type of shapes contained within this DES, or how those
      shapes are to be used.
      25 BCS-A characters.
      Values:
      - IMAGE_SHAPE: Shape of the original delivered image
      - CLOUD_SHAPES: Shapes of detected clouds within the image
      - USER_DEF_SHAPES: Miscellaneous shapes defined by the data provider
      - *_LineSample suffix: Indicates LineSample coordinates instead of WGS 84

  - id: shape_class
    type: str
    size: 10
    encoding: BCS-A
    doc: |
      Type of Shapes (SHAPE_CLASS)
      Type of shapes contained within this shapefile.
      10 BCS-A characters.
      Values: NULL SHAPE, POINT, POLYLINE, POLYGON, MULTIPOINT, POINTZ,
      POLYLINEZ, POLYGONZ, MULTPOINTZ, POINTM, POLYLINEM, POLYGONM,
      MULTPOINTM, MULTIPATCH

  - id: cc_source
    type: str
    size: 18
    encoding: BCS-A
    if: _root._io.size >= 80
    doc: |
      Cloud Cover Source (CC_SOURCE)
      Source sensor(s) for determining cloud cover.
      18 BCS-A characters.
      Only present if SHAPE_USE is CLOUD_SHAPES.
      Values: PAN, MS, SWIR, CAVIS (or comma-separated combination)

  - id: shape1_name
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Name of First Component File (SHAPE1_NAME)
      Name of first component file of the shapefile.
      3 BCS-A characters.
      Values: SHP, SHX, DBF

  - id: shape1_start
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Offset to First Component File (SHAPE1_START)
      Offset to the start of the first component file, in bytes,
      from the start of the DES user-defined data.
      6 BCS-N characters.
      Range: 000000 to 999999

  - id: shape2_name
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Name of Second Component File (SHAPE2_NAME)
      Name of second component file of the shapefile.
      3 BCS-A characters.
      Values: SHP, SHX, DBF

  - id: shape2_start
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Offset to Second Component File (SHAPE2_START)
      Offset to the start of the second component file, in bytes,
      from the start of the DES user-defined data.
      6 BCS-N characters.
      Range: 000000 to 999999

  - id: shape3_name
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Name of Third Component File (SHAPE3_NAME)
      Name of third component file of the shapefile.
      3 BCS-A characters.
      Values: SHP, SHX, DBF

  - id: shape3_start
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Offset to Third Component File (SHAPE3_START)
      Offset to the start of the third component file, in bytes,
      from the start of the DES user-defined data.
      6 BCS-N characters.
      Range: 000000 to 999999

