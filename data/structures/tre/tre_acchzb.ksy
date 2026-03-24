meta:
  id: tre_acchzb
  title: Horizontal Accuracy Data TRE
  endian: be

doc: |
  ACCHZB TRE - Horizontal Accuracy Data
  
  Provides horizontal accuracy information for imagery products.
  Contains accuracy values and boundary polygons defining regions
  of consistent horizontal accuracy.
  
  Reference: STDI-0002 Volume 1, Appendix P, Section P.3.2.6.2, Table P-11

seq:
  - id: NUM_ACHZ
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Number of Horizontal Accuracy Regions
      2 BCS-N integer, range 01-99.

  - id: ACHZ_DATA
    type: achz_record
    repeat: expr
    repeat-expr: NUM_ACHZ.to_i
    doc: Horizontal accuracy region records.

types:
  achz_record:
    seq:
      - id: UNIAAH
        type: str
        size: 3
        encoding: BCS-A
        doc: |
          Unit of Measure for AAH
          3 BCS-A. Spaces if AAH not provided.

      - id: AAH
        type: str
        size: 5
        encoding: BCS-N
        if: UNIAAH != "   "
        doc: |
          Absolute Horizontal Accuracy
          5 BCS-N integer.

      - id: UNIAPH
        type: str
        size: 3
        encoding: BCS-A
        doc: |
          Unit of Measure for APH
          3 BCS-A. Spaces if APH not provided.

      - id: APH
        type: str
        size: 5
        encoding: BCS-N
        if: UNIAPH != "   "
        doc: |
          Point-to-Point Horizontal Accuracy
          5 BCS-N integer.

      - id: NUM_PTS
        type: str
        size: 3
        encoding: BCS-N
        doc: |
          Number of Points in Bounding Polygon
          3 BCS-N integer, range 000-999.

      - id: POINTS
        type: accuracy_point
        repeat: expr
        repeat-expr: NUM_PTS.to_i
        doc: Bounding polygon vertices.

  accuracy_point:
    seq:
      - id: LON
        type: str
        size: 15
        encoding: BCS-A
        doc: |
          Longitude
          15 BCS-A.

      - id: LAT
        type: str
        size: 15
        encoding: BCS-A
        doc: |
          Latitude
          15 BCS-A.
