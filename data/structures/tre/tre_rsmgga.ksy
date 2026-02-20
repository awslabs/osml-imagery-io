meta:
  id: tre_rsmgga
  title: RSM Ground-to-Image Grid TRE
  endian: be

doc: |
  RSMGGA TRE - Replacement Sensor Model Ground-to-Image Grid
  
  Provides ground-to-image grid data for a single ground section of the RSM.
  Contains section numbers, fit error, interpolation order, plane definitions,
  and grid point coordinates for mapping ground coordinates to image coordinates.
  
  CEL: 390-99988 bytes (variable based on number of grid points and PLANES)
  
  Reference: STDI-0002 Volume 1, Appendix U - RSM

seq:
  - id: IID
    type: str
    size: 80
    encoding: BCS-A
    doc: |
      Image Identifier
      80 BCS-A characters identifying the image.

  - id: EDITION
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      RSM Image Support Data Edition
      40 BCS-A characters identifying the edition.

  - id: GGRSN
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Ground Row Section Number
      3 BCS-NPI positive integer (1 to GRNIS).

  - id: GGCSN
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Ground Column Section Number
      3 BCS-NPI positive integer (1 to GCNIS).

  - id: GGRFEP
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row Fit Error in Pixels
      21 BCS-N real number.

  - id: GGCFEP
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column Fit Error in Pixels
      21 BCS-N real number.

  - id: INTORD
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Interpolation Order
      1 BCS-NPI digit: 1 = bilinear, 3 = bicubic.

  - id: NPLN
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Planes
      3 BCS-NPI positive integer.

  - id: DELTAZ
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Delta Z Between Planes
      21 BCS-N real number (ground units).

  - id: DELTAX
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Delta X Between Grid Points
      21 BCS-N real number (ground units).

  - id: DELTAY
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Delta Y Between Grid Points
      21 BCS-N real number (ground units).

  - id: ZPLN1
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Z Coordinate of First Plane
      21 BCS-N real number (ground units).

  - id: XIPLN1
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      X Coordinate of Initial Point in First Plane
      21 BCS-N real number (ground units).

  - id: YIPLN1
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Y Coordinate of Initial Point in First Plane
      21 BCS-N real number (ground units).

  - id: REFROW
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Reference Row for Grid Origin
      9 BCS-N real number (image pixels).

  - id: REFCOL
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Reference Column for Grid Origin
      9 BCS-N real number (image pixels).

  - id: TNUMRD
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Total Number of Row Delta Values
      2 BCS-NPI positive integer.

  - id: TNUMCD
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Total Number of Column Delta Values
      2 BCS-NPI positive integer.

  - id: FNUMRD
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Field Size for Row Delta Values
      1 BCS-NPI digit (bytes per value).

  - id: FNUMCD
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Field Size for Column Delta Values
      1 BCS-NPI digit (bytes per value).

  - id: DELTA_ORIGIN
    type: delta_origin_t
    repeat: expr
    repeat-expr: NPLN.to_i
    doc: |
      Delta Origin Values for Each Plane
      NPLN sets of row and column origin deltas.

  - id: PLANES
    type: plane_t
    repeat: expr
    repeat-expr: NPLN.to_i
    doc: |
      Grid Data for Each Plane
      NPLN plane records containing grid point data.

types:
  delta_origin_t:
    seq:
      - id: IXO
        type: str
        size: 4
        encoding: BCS-NPI
        doc: |
          X Origin Delta for Plane
          4 BCS-NPI integer (grid units).

      - id: IYO
        type: str
        size: 4
        encoding: BCS-NPI
        doc: |
          Y Origin Delta for Plane
          4 BCS-NPI integer (grid units).

  plane_t:
    seq:
      - id: NXPTS
        type: str
        size: 3
        encoding: BCS-NPI
        doc: |
          Number of X Grid Points in Plane
          3 BCS-NPI positive integer.

      - id: NYPTS
        type: str
        size: 3
        encoding: BCS-NPI
        doc: |
          Number of Y Grid Points in Plane
          3 BCS-NPI positive integer.

      - id: GRID_POINTS
        type: grid_point_t(_parent.FNUMRD.to_i, _parent.FNUMCD.to_i)
        repeat: expr
        repeat-expr: NXPTS.to_i * NYPTS.to_i
        doc: |
          Grid Point Coordinates
          NXPTS * NYPTS grid points with row and column deltas.

  grid_point_t:
    params:
      - id: ROW_SIZE
        type: u1
      - id: COL_SIZE
        type: u1
    seq:
      - id: RCOORD
        type: str
        size: row_size
        encoding: BCS-N
        doc: |
          Row Coordinate Delta
          Variable size BCS-N integer (FNUMRD bytes).

      - id: CCOORD
        type: str
        size: col_size
        encoding: BCS-N
        doc: |
          Column Coordinate Delta
          Variable size BCS-N integer (FNUMCD bytes).
