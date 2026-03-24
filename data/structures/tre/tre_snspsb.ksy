meta:
  id: tre_snspsb
  title: Sensor Parameters TRE
  endian: be

doc: |
  SNSPSB TRE - Sensor Parameters
  
  Provides detailed sensor parameters including boundary polygons,
  band information, resolution, platform/instrument identification,
  attitude, and auxiliary data for each sensor in the image.
  
  Reference: STDI-0002 Volume 1, Appendix P, Section P.3.2.7.2, Table P-13

seq:
  - id: NUM_SNS
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Number of Sensors
      2 BCS-N integer.

  - id: SENSORS
    type: sensor_record
    repeat: expr
    repeat-expr: NUM_SNS.to_i
    doc: Sensor records.

types:
  sensor_record:
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
      - id: NUM_BND
        type: str
        size: 2
        encoding: BCS-N
        doc: Number of Bands (2 BCS-N).
      - id: BANDS
        type: band_record
        repeat: expr
        repeat-expr: NUM_BND.to_i
        doc: Band records.
      - id: UNIRES
        type: str
        size: 3
        encoding: BCS-A
        doc: Unit of Resolution (3 BCS-A).
      - id: REX
        type: str
        size: 6
        encoding: BCS-N
        doc: Resolution in X (6 BCS-N real).
      - id: REY
        type: str
        size: 6
        encoding: BCS-N
        doc: Resolution in Y (6 BCS-N real).
      - id: GSX
        type: str
        size: 6
        encoding: BCS-N
        doc: Ground Sample Distance X (6 BCS-N real).
      - id: GSY
        type: str
        size: 6
        encoding: BCS-N
        doc: Ground Sample Distance Y (6 BCS-N real).
      - id: GSL
        type: str
        size: 12
        encoding: BCS-A
        doc: GSD Location (12 BCS-A).
      - id: PLTFM
        type: str
        size: 8
        encoding: BCS-A
        doc: Platform (8 BCS-A).
      - id: INS
        type: str
        size: 8
        encoding: BCS-A
        doc: Instrument (8 BCS-A).
      - id: MOD
        type: str
        size: 4
        encoding: BCS-A
        doc: Mode (4 BCS-A).
      - id: PRL
        type: str
        size: 5
        encoding: BCS-A
        doc: Processing Level (5 BCS-A).
      - id: SID
        type: str
        size: 10
        encoding: BCS-A
        doc: Scene ID (10 BCS-A).
      - id: ACT
        type: str
        size: 18
        encoding: BCS-A
        doc: Acquisition Time (18 BCS-A).
      - id: UNINOA
        type: str
        size: 3
        encoding: BCS-A
        doc: Unit of Nominal Obliquity Angle (3 BCS-A).
      - id: NOA
        type: str
        size: 7
        encoding: BCS-N
        if: UNINOA != "   "
        doc: Nominal Obliquity Angle (7 BCS-N real).
      - id: UNIANG
        type: str
        size: 3
        encoding: BCS-A
        doc: Unit of Angle to North (3 BCS-A).
      - id: ANG
        type: str
        size: 7
        encoding: BCS-N
        if: UNIANG != "   "
        doc: Angle to North (7 BCS-N real).
      - id: UNIALT
        type: str
        size: 3
        encoding: BCS-A
        doc: Unit of Altitude (3 BCS-A).
      - id: ALT
        type: str
        size: 9
        encoding: BCS-N
        if: UNIALT != "   "
        doc: Altitude (9 BCS-N real).
      - id: LONSCC
        type: str
        size: 10
        encoding: BCS-N
        doc: Longitude of Scene Center (10 BCS-N real).
      - id: LATSCC
        type: str
        size: 10
        encoding: BCS-N
        doc: Latitude of Scene Center (10 BCS-N real).
      - id: UNISAE
        type: str
        size: 3
        encoding: BCS-A
        doc: Unit of Sun Azimuth/Elevation (3 BCS-A).
      - id: SAZ
        type: str
        size: 7
        encoding: BCS-N
        if: UNISAE != "   "
        doc: Sun Azimuth (7 BCS-N real).
      - id: SEL
        type: str
        size: 7
        encoding: BCS-N
        if: UNISAE != "   "
        doc: Sun Elevation (7 BCS-N real).
      - id: UNIRPY
        type: str
        size: 3
        encoding: BCS-A
        doc: Unit of Roll/Pitch/Yaw (3 BCS-A).
      - id: ROL
        type: str
        size: 7
        encoding: BCS-N
        if: UNIRPY != "   "
        doc: Roll (7 BCS-N real).
      - id: PIT
        type: str
        size: 7
        encoding: BCS-N
        if: UNIRPY != "   "
        doc: Pitch (7 BCS-N real).
      - id: YAW
        type: str
        size: 7
        encoding: BCS-N
        if: UNIRPY != "   "
        doc: Yaw (7 BCS-N real).
      - id: UNIPXT
        type: str
        size: 3
        encoding: BCS-A
        doc: Unit of Pixel Time (3 BCS-A).
      - id: PXT
        type: str
        size: 14
        encoding: BCS-N
        if: UNIPXT != "   "
        doc: Pixel Time (14 BCS-N real).
      - id: UNISPE
        type: str
        size: 7
        encoding: BCS-A
        doc: Unit of Speed (7 BCS-A).
      - id: ROS
        type: str
        size: 22
        encoding: BCS-N
        if: UNISPE != "       "
        doc: Roll Speed (22 BCS-N real).
      - id: PIS
        type: str
        size: 22
        encoding: BCS-N
        if: UNISPE != "       "
        doc: Pitch Speed (22 BCS-N real).
      - id: YAS
        type: str
        size: 22
        encoding: BCS-N
        if: UNISPE != "       "
        doc: Yaw Speed (22 BCS-N real).
      - id: NUM_AUX
        type: str
        size: 3
        encoding: BCS-N
        doc: Number of Auxiliary Parameters (3 BCS-N).
      - id: AUX_PARAMS
        type: aux_param
        repeat: expr
        repeat-expr: NUM_AUX.to_i
        doc: Auxiliary parameter records.

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

  band_record:
    seq:
      - id: BID
        type: str
        size: 5
        encoding: BCS-A
        doc: Band ID (5 BCS-A).
      - id: WS1
        type: str
        size: 5
        encoding: BCS-N
        doc: Wavelength Start (5 BCS-N integer).
      - id: WS2
        type: str
        size: 5
        encoding: BCS-N
        doc: Wavelength Stop (5 BCS-N integer).

  aux_param:
    seq:
      - id: API
        type: str
        size: 20
        encoding: BCS-A
        doc: Auxiliary Parameter ID (20 BCS-A).
      - id: APF
        type: str
        size: 1
        encoding: BCS-A
        doc: Auxiliary Parameter Format (1 BCS-A, I/R/A).
      - id: UNIAPX
        type: str
        size: 7
        encoding: BCS-A
        doc: Unit of Auxiliary Parameter (7 BCS-A).
      - id: APN
        type: str
        size: 10
        encoding: BCS-N
        if: APF == "I"
        doc: Auxiliary Parameter Integer Value (10 BCS-N).
      - id: APR
        type: str
        size: 20
        encoding: BCS-N
        if: APF == "R"
        doc: Auxiliary Parameter Real Value (20 BCS-N).
      - id: APA
        type: str
        size: 20
        encoding: BCS-A
        if: APF == "A"
        doc: Auxiliary Parameter ASCII Value (20 BCS-A).
