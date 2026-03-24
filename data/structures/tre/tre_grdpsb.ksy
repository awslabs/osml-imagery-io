meta:
  id: tre_grdpsb
  title: Grid Reference Data TRE
  endian: be

doc: |
  GRDPSB TRE - Grid Reference Data
  
  Provides grid reference data for imagery products. Contains
  grid records with elevation, angular density, and origin values.
  
  Reference: STDI-0002 Volume 1, Appendix P, Section P.3.2.5.3, Table P-4

seq:
  - id: NUM_GRDS
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Number of Grid Records
      2 BCS-N integer, minimum 1.

  - id: GRDS
    type: grid_record
    repeat: expr
    repeat-expr: NUM_GRDS.to_i
    doc: Grid reference records.

types:
  grid_record:
    seq:
      - id: ZVL
        type: str
        size: 10
        encoding: BCS-N
        doc: |
          Z Value (elevation) in meters
          10 BCS-N real.

      - id: BAD
        type: str
        size: 10
        encoding: BCS-A
        doc: |
          Band ID
          10 BCS-A.

      - id: LOD
        type: str
        size: 12
        encoding: BCS-N
        doc: |
          Longitude Density
          12 BCS-N real.

      - id: LAD
        type: str
        size: 12
        encoding: BCS-N
        doc: |
          Latitude Density
          12 BCS-N real.

      - id: LSO
        type: str
        size: 11
        encoding: BCS-N
        doc: |
          Longitude of Reference Origin
          11 BCS-N real.

      - id: PSO
        type: str
        size: 11
        encoding: BCS-N
        doc: |
          Latitude of Reference Origin
          11 BCS-N real.
