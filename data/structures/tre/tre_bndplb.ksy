meta:
  id: tre_bndplb
  title: Boundary Polygon TRE
  endian: be

doc: |
  BNDPLB TRE - Boundary Polygon
  
  Defines a boundary polygon for an image using a series of
  longitude/latitude coordinate pairs.
  
  Reference: STDI-0002 Volume 1, Appendix P, Section P.3.2.5.7.1, Table P-9

seq:
  - id: NUM_PTS
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Number of Points in Bounding Polygon
      4 BCS-N integer, range 0004-3332.

  - id: POINTS
    type: boundary_point
    repeat: expr
    repeat-expr: NUM_PTS.to_i
    doc: Boundary polygon vertices.

types:
  boundary_point:
    seq:
      - id: LON
        type: str
        size: 15
        encoding: BCS-N
        doc: |
          Longitude
          15 BCS-N real.

      - id: LAT
        type: str
        size: 15
        encoding: BCS-N
        doc: |
          Latitude
          15 BCS-N real.
