meta:
  id: tre_accpob
  title: Positional Accuracy Data TRE
  endian: be

doc: |
  ACCPOB TRE - Positional Accuracy Data
  
  Provides positional accuracy information for imagery products.
  Contains absolute and point-to-point accuracy values for both
  horizontal and vertical dimensions, along with bounding polygons.
  
  Reference: STDI-0002 Volume 1, Appendix P, Section P.3.2.6.1, Table P-10

seq:
  - id: NUM_ACPO
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Number of Positional Accuracy Regions
      2 BCS-N integer, range 01-99.

  - id: ACPO_DATA
    type: acpo_record
    repeat: expr
    repeat-expr: NUM_ACPO.to_i
    doc: Positional accuracy region records.

types:
  acpo_record:
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

      - id: UNIAAV
        type: str
        size: 3
        encoding: BCS-A
        doc: |
          Unit of Measure for AAV
          3 BCS-A. Spaces if AAV not provided.

      - id: AAV
        type: str
        size: 5
        encoding: BCS-N
        if: UNIAAV != "   "
        doc: |
          Absolute Vertical Accuracy
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

      - id: UNIAPV
        type: str
        size: 3
        encoding: BCS-A
        doc: |
          Unit of Measure for APV
          3 BCS-A. Spaces if APV not provided.

      - id: APV
        type: str
        size: 5
        encoding: BCS-N
        if: UNIAPV != "   "
        doc: |
          Point-to-Point Vertical Accuracy
          5 BCS-N integer.

      - id: NUM_PTS
        type: str
        size: 3
        encoding: BCS-N
        doc: |
          Number of Points in Bounding Polygon
          3 BCS-N integer.

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
