meta:
  id: tre_mstgta
  title: Multiple Scene Target TRE
  endian: be

doc: |
  MSTGTA TRE - Mission Target Information
  
  Provides information from the collection plan associated with the
  image, identifying specific targets contained within the image.
  Contains target identification, location, priority, and collection
  information.
  
  Fixed length: 101 bytes.
  
  Reference: STDI-0002 Volume 1, Appendix E, Section E.3.9, Table E-16

seq:
  - id: TGT_NUM
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Target Number
      5 BCS-N integer, range 00000-99999.

  - id: TGT_ID
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Target Identifier
      12 BCS-A.

  - id: TGT_BE
    type: str
    size: 15
    encoding: BCS-A
    doc: |
      Target Basic Encyclopedia Number
      15 BCS-A.

  - id: TGT_PRI
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Target Priority
      3 BCS-N integer, range 001-999.

  - id: TGT_REQ
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Target Request
      12 BCS-A.

  - id: TGT_LTIOV
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Target Latest Time Information of Value
      12 BCS-A.

  - id: TGT_TYPE
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Target Type
      1 BCS-N integer.

  - id: TGT_COLL
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Target Collateral
      1 BCS-N integer.

  - id: TGT_CAT
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Target Category
      5 BCS-N integer, range 10000-99999.

  - id: TGT_UTC
    type: str
    size: 7
    encoding: BCS-A
    doc: |
      Target UTC
      7 BCS-A.

  - id: TGT_ELEV
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Target Elevation
      6 BCS-N integer, range -1000 to 30000.

  - id: TGT_ELEV_UNIT
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Target Elevation Unit
      1 BCS-A.

  - id: TGT_LOC
    type: str
    size: 21
    encoding: BCS-A
    doc: |
      Target Location
      21 BCS-A.
