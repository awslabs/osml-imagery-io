meta:
  id: tre_accvtb
  title: Vertical Accuracy Data TRE
  endian: be

doc: |
  ACCVTB TRE - Vertical Accuracy Data
  
  Provides vertical accuracy information for imagery products.
  Contains accuracy values and boundary polygons defining regions
  of consistent vertical accuracy.
  
  Reference: STDI-0002 Volume 1, Appendix P, Section P.3.2.6.3, Table P-12

seq:
  - id: NUM_ACVT
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Number of Vertical Accuracy Regions
      2 BCS-N integer, range 01-99.

  - id: ACVT_DATA
    type: acvt_record
    repeat: expr
    repeat-expr: NUM_ACVT.to_i
    doc: Vertical accuracy region records.

types:
  acvt_record:
    seq:
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
