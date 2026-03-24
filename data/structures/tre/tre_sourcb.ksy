meta:
  id: tre_sourcb
  title: Source Data TRE
  endian: be

doc: |
  SOURCB TRE - Source Data
  
  Provides detailed source data information for imagery products
  including boundary polygons, product references, datum/ellipsoid
  information, projection parameters, and grid data for each source.
  
  This is one of the most complex TREs with deeply nested loops
  and conditional fields.
  
  Reference: STDI-0002 Volume 1, Appendix P, Section P.3.2.7.3, Table P-14

seq:
  - id: IS_SCA
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Image Scale
      9 BCS-N integer.

  - id: CPATCH
    type: str
    size: 10
    encoding: BCS-A
    doc: |
      Compilation Patch
      10 BCS-A.

  - id: NUM_SOUR
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Number of Sources
      2 BCS-N integer, minimum 1.

  - id: SOURCES
    type: source_record
    repeat: expr
    repeat-expr: NUM_SOUR.to_i
    doc: Source records.

types:
  source_record:
    seq:
      - id: NUM_BP
        type: str
        size: 2
        encoding: BCS-N
        doc: Number of Boundary Polygons (2 BCS-N).
      - id: POLYGONS
        type: boundary_polygon
        repeat: expr
        repeat-expr: NUM_BP.to_i
        doc: Boundary polygons.
      - id: PRT
        type: str
        size: 10
        encoding: BCS-A
        doc: Product Type (10 BCS-A).
      - id: URF
        type: str
        size: 20
        encoding: BCS-A
        doc: Unit Reference Frame (20 BCS-A).
      - id: EDN
        type: str
        size: 7
        encoding: BCS-A
        doc: Edition Number (7 BCS-A).
      - id: NAM
        type: str
        size: 20
        encoding: BCS-A
        doc: Name (20 BCS-A).
      - id: CDP
        type: str
        size: 3
        encoding: BCS-N
        doc: Compilation Date Precision (3 BCS-N integer).
      - id: CDV
        type: str
        size: 8
        encoding: BCS-A
        doc: Compilation Date Version (8 BCS-A).
      - id: CDV27
        type: str
        size: 8
        encoding: BCS-A
        doc: Compilation Date Version 27 (8 BCS-A).
      - id: SRN
        type: str
        size: 80
        encoding: BCS-A
        doc: Source Reference Number (80 BCS-A).
      - id: SCA
        type: str
        size: 9
        encoding: BCS-N
        doc: Source Scale (9 BCS-N integer).
      - id: UNISQU
        type: str
        size: 3
        encoding: BCS-A
        doc: Unit of Source Quality (3 BCS-A).
      - id: SQU
        type: str
        size: 10
        encoding: BCS-N
        if: UNISQU != "   "
        doc: Source Quality (10 BCS-N integer).
      - id: UNIPCI
        type: str
        size: 3
        encoding: BCS-A
        doc: Unit of PCI (3 BCS-A).
      - id: PCI
        type: str
        size: 4
        encoding: BCS-N
        if: UNIPCI != "   "
        doc: PCI Value (4 BCS-N integer).
      - id: WPC
        type: str
        size: 3
        encoding: BCS-N
        doc: WPC (3 BCS-N integer).
      - id: NST
        type: str
        size: 3
        encoding: BCS-N
        doc: NST (3 BCS-N integer).
      - id: UNIHKE
        type: str
        size: 3
        encoding: BCS-A
        doc: Unit of HKE (3 BCS-A).
      - id: HKE
        type: str
        size: 6
        encoding: BCS-N
        if: UNIHKE != "   "
        doc: HKE Value (6 BCS-N integer).
      - id: LONHKE
        type: str
        size: 15
        encoding: BCS-N
        if: UNIHKE != "   "
        doc: Longitude of HKE (15 BCS-N real).
      - id: LATHKE
        type: str
        size: 15
        encoding: BCS-N
        if: UNIHKE != "   "
        doc: Latitude of HKE (15 BCS-N real).
      - id: QSS
        type: str
        size: 1
        encoding: BCS-A
        doc: Quality Source Status (1 BCS-A).
      - id: QOD
        type: str
        size: 1
        encoding: BCS-A
        doc: Quality of Data (1 BCS-A).
      - id: QLE
        type: str
        size: 80
        encoding: BCS-A
        doc: Quality Legend (80 BCS-A).
      - id: CPY
        type: str
        size: 80
        encoding: BCS-A
        doc: Copyright (80 BCS-A).
      - id: NMI
        type: str
        size: 2
        encoding: BCS-N
        doc: Number of Map Items (2 BCS-N integer).
      - id: MAP_ITEMS
        type: map_item
        repeat: expr
        repeat-expr: NMI.to_i
        doc: Map item records.
      - id: NLI
        type: str
        size: 2
        encoding: BCS-N
        doc: Number of Line Items (2 BCS-N integer).
      - id: LINE_ITEMS
        type: line_item
        repeat: expr
        repeat-expr: NLI.to_i
        doc: Line item records.
      - id: DAG
        type: str
        size: 80
        encoding: BCS-A
        doc: Datum Name (80 BCS-A).
      - id: DCD
        type: str
        size: 4
        encoding: BCS-A
        doc: Datum Code (4 BCS-A).
      - id: ELL
        type: str
        size: 80
        encoding: BCS-A
        doc: Ellipsoid Name (80 BCS-A).
      - id: ELC
        type: str
        size: 3
        encoding: BCS-A
        doc: Ellipsoid Code (3 BCS-A).
      - id: DVR
        type: str
        size: 80
        encoding: BCS-A
        doc: Vertical Datum Reference (80 BCS-A).
      - id: VDCDVR
        type: str
        size: 4
        encoding: BCS-A
        doc: Vertical Datum Code (4 BCS-A).
      - id: SDA
        type: str
        size: 80
        encoding: BCS-A
        doc: Sounding Datum (80 BCS-A).
      - id: VDCSDA
        type: str
        size: 4
        encoding: BCS-A
        doc: Sounding Datum Code (4 BCS-A).
      - id: PRN
        type: str
        size: 80
        encoding: BCS-A
        doc: Projection Name (80 BCS-A).
      - id: PCO
        type: str
        size: 2
        encoding: BCS-A
        doc: Projection Code (2 BCS-A).
      - id: NUM_PRJ
        type: str
        size: 1
        encoding: BCS-N
        doc: Number of Projection Parameters (1 BCS-N integer).
      - id: PRJ_PARAMS
        type: str
        size: 15
        encoding: BCS-N
        repeat: expr
        repeat-expr: NUM_PRJ.to_i
        doc: Projection Parameters (15 BCS-N real each).
      - id: XOR
        type: str
        size: 15
        encoding: BCS-N
        doc: X Origin (15 BCS-N integer).
      - id: YOR
        type: str
        size: 15
        encoding: BCS-N
        doc: Y Origin (15 BCS-N integer).
      - id: GRD
        type: str
        size: 3
        encoding: BCS-A
        doc: Grid Code (3 BCS-A).
      - id: GRN
        type: str
        size: 80
        encoding: BCS-A
        doc: Grid Name (80 BCS-A).
      - id: ZNA
        type: str
        size: 4
        encoding: BCS-N
        doc: Zone Number (4 BCS-N integer).
      - id: NIN
        type: str
        size: 2
        encoding: BCS-N
        doc: Number of Intersection Records (2 BCS-N integer).
      - id: INTERSECTIONS
        type: intersection_record
        repeat: expr
        repeat-expr: NIN.to_i
        doc: Intersection records.

  boundary_polygon:
    seq:
      - id: NUM_PTS
        type: str
        size: 3
        encoding: BCS-N
        doc: Number of Points (3 BCS-N).
      - id: POINTS
        type: geo_point
        repeat: expr
        repeat-expr: NUM_PTS.to_i
        doc: Polygon vertices.

  geo_point:
    seq:
      - id: LON
        type: str
        size: 15
        encoding: BCS-N
        doc: Longitude (15 BCS-N real).
      - id: LAT
        type: str
        size: 15
        encoding: BCS-N
        doc: Latitude (15 BCS-N real).

  map_item:
    seq:
      - id: CDV30
        type: str
        size: 8
        encoding: BCS-A
        doc: Compilation Date Version 30 (8 BCS-A).
      - id: UNIRAT
        type: str
        size: 3
        encoding: BCS-A
        doc: Unit of Rate (3 BCS-A).
      - id: RAT
        type: str
        size: 8
        encoding: BCS-N
        doc: Rate (8 BCS-N real).
      - id: UNIGMA
        type: str
        size: 3
        encoding: BCS-A
        doc: Unit of GMA (3 BCS-A).
      - id: GMA
        type: str
        size: 8
        encoding: BCS-N
        doc: GMA Value (8 BCS-N real).
      - id: LONGMA
        type: str
        size: 15
        encoding: BCS-N
        doc: Longitude of GMA (15 BCS-N real).
      - id: LATGMA
        type: str
        size: 15
        encoding: BCS-N
        doc: Latitude of GMA (15 BCS-N real).
      - id: UNIGCA
        type: str
        size: 3
        encoding: BCS-A
        doc: Unit of GCA (3 BCS-A).
      - id: GCA
        type: str
        size: 8
        encoding: BCS-N
        if: UNIGCA != "   "
        doc: GCA Value (8 BCS-N real).

  line_item:
    seq:
      - id: BAD
        type: str
        size: 10
        encoding: BCS-A
        doc: Band ID (10 BCS-A).

  intersection_record:
    seq:
      - id: INT
        type: str
        size: 10
        encoding: BCS-A
        doc: Intersection Type (10 BCS-A).
      - id: INS_SCA
        type: str
        size: 9
        encoding: BCS-N
        doc: Intersection Scale (9 BCS-N integer).
      - id: NTL
        type: str
        size: 15
        encoding: BCS-N
        doc: North Top Left (15 BCS-N real).
      - id: TTL
        type: str
        size: 15
        encoding: BCS-N
        doc: Top Top Left (15 BCS-N real).
      - id: NVL
        type: str
        size: 15
        encoding: BCS-N
        doc: North Vertical Left (15 BCS-N real).
      - id: TVL
        type: str
        size: 15
        encoding: BCS-N
        doc: Top Vertical Left (15 BCS-N real).
      - id: NTR
        type: str
        size: 15
        encoding: BCS-N
        doc: North Top Right (15 BCS-N real).
      - id: TTR
        type: str
        size: 15
        encoding: BCS-N
        doc: Top Top Right (15 BCS-N real).
      - id: NVR
        type: str
        size: 15
        encoding: BCS-N
        doc: North Vertical Right (15 BCS-N real).
      - id: TVR
        type: str
        size: 15
        encoding: BCS-N
        doc: Top Vertical Right (15 BCS-N real).
      - id: NRL
        type: str
        size: 15
        encoding: BCS-N
        doc: North Right Left (15 BCS-N real).
      - id: TRL
        type: str
        size: 15
        encoding: BCS-N
        doc: Top Right Left (15 BCS-N real).
      - id: NSL
        type: str
        size: 15
        encoding: BCS-N
        doc: North South Left (15 BCS-N real).
      - id: TSL
        type: str
        size: 15
        encoding: BCS-N
        doc: Top South Left (15 BCS-N real).
      - id: NRR
        type: str
        size: 15
        encoding: BCS-N
        doc: North Right Right (15 BCS-N real).
      - id: TRR
        type: str
        size: 15
        encoding: BCS-N
        doc: Top Right Right (15 BCS-N real).
      - id: NSR
        type: str
        size: 15
        encoding: BCS-N
        doc: North South Right (15 BCS-N real).
      - id: TSR
        type: str
        size: 15
        encoding: BCS-N
        doc: Top South Right (15 BCS-N real).
