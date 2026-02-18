meta:
  id: tre_rsmgga
  title: RSM Ground-to-Image Grid TRE
  endian: be

doc: |
  RSMGGA TRE - Replacement Sensor Model Ground-to-Image Grid
  
  Provides ground-to-image grid data for a single ground section of the RSM.
  Contains section numbers, fit error, interpolation order, plane definitions,
  and grid point coordinates for mapping ground coordinates to image coordinates.
  
  CEL: 390-99988 bytes (variable based on number of grid points and planes)
  
  Reference: STDI-0002 Volume 1, Appendix U - RSM

seq:
  - id: iid
    type: str
    size: 80
    encoding: BCS-A
    doc: |
      Image Identifier
      80 BCS-A characters identifying the image.

  - id: edition
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      RSM Image Support Data Edition
      40 BCS-A characters identifying the edition.

  - id: ggrsn
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Ground Row Section Number
      3 BCS-NPI positive integer (1 to GRNIS).

  - id: ggcsn
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Ground Column Section Number
      3 BCS-NPI positive integer (1 to GCNIS).

  - id: ggrfep
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Row Fit Error in Pixels
      21 BCS-N real number.

  - id: ggcfep
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Ground Column Fit Error in Pixels
      21 BCS-N real number.

  - id: intord
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Interpolation Order
      1 BCS-NPI digit: 1 = bilinear, 3 = bicubic.

  - id: npln
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Planes
      3 BCS-NPI positive integer.

  - id: deltaz
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Delta Z Between Planes
      21 BCS-N real number (ground units).

  - id: deltax
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Delta X Between Grid Points
      21 BCS-N real number (ground units).

  - id: deltay
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Delta Y Between Grid Points
      21 BCS-N real number (ground units).

  - id: zpln1
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Z Coordinate of First Plane
      21 BCS-N real number (ground units).

  - id: xipln1
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      X Coordinate of Initial Point in First Plane
      21 BCS-N real number (ground units).

  - id: yipln1
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Y Coordinate of Initial Point in First Plane
      21 BCS-N real number (ground units).

  - id: refrow
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Reference Row for Grid Origin
      9 BCS-N real number (image pixels).

  - id: refcol
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Reference Column for Grid Origin
      9 BCS-N real number (image pixels).

  - id: tnumrd
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Total Number of Row Delta Values
      2 BCS-NPI positive integer.

  - id: tnumcd
    type: str
    size: 2
    encoding: BCS-NPI
    doc: |
      Total Number of Column Delta Values
      2 BCS-NPI positive integer.

  - id: fnumrd
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Field Size for Row Delta Values
      1 BCS-NPI digit (bytes per value).

  - id: fnumcd
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Field Size for Column Delta Values
      1 BCS-NPI digit (bytes per value).

  - id: delta_origin
    type: delta_origin_t
    repeat: expr
    repeat-expr: npln.to_i
    doc: |
      Delta Origin Values for Each Plane
      NPLN sets of row and column origin deltas.

  - id: planes
    type: plane_t
    repeat: expr
    repeat-expr: npln.to_i
    doc: |
      Grid Data for Each Plane
      NPLN plane records containing grid point data.

types:
  delta_origin_t:
    seq:
      - id: ixo
        type: str
        size: 4
        encoding: BCS-NPI
        doc: |
          X Origin Delta for Plane
          4 BCS-NPI integer (grid units).

      - id: iyo
        type: str
        size: 4
        encoding: BCS-NPI
        doc: |
          Y Origin Delta for Plane
          4 BCS-NPI integer (grid units).

  plane_t:
    seq:
      - id: nxpts
        type: str
        size: 3
        encoding: BCS-NPI
        doc: |
          Number of X Grid Points in Plane
          3 BCS-NPI positive integer.

      - id: nypts
        type: str
        size: 3
        encoding: BCS-NPI
        doc: |
          Number of Y Grid Points in Plane
          3 BCS-NPI positive integer.

      - id: grid_points
        type: grid_point_t(_parent.fnumrd.to_i, _parent.fnumcd.to_i)
        repeat: expr
        repeat-expr: nxpts.to_i * nypts.to_i
        doc: |
          Grid Point Coordinates
          NXPTS * NYPTS grid points with row and column deltas.

  grid_point_t:
    params:
      - id: row_size
        type: u1
      - id: col_size
        type: u1
    seq:
      - id: rcoord
        type: str
        size: row_size
        encoding: BCS-N
        doc: |
          Row Coordinate Delta
          Variable size BCS-N integer (FNUMRD bytes).

      - id: ccoord
        type: str
        size: col_size
        encoding: BCS-N
        doc: |
          Column Coordinate Delta
          Variable size BCS-N integer (FNUMCD bytes).
