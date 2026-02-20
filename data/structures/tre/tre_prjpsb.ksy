meta:
  id: tre_prjpsb
  title: Projection Parameters TRE
  endian: be

doc: |
  PRJPSB TRE - Projection Parameters Tagged Record Extension
  
  Provides map projection parameters for NITF images including
  projection type, false origins, and projection-specific parameters.
  
  Reference: STDI-0002 Volume 1, Appendix P - GEOSDE

seq:
  - id: PRJ
    type: str
    size: 80
    encoding: ECS-A
    doc: |
      Projection Name (PRJ)
      80 ECS-A. Full name of the map projection.

  - id: PJC
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Projection Code (PJC)
      2 BCS-A. Two-character code identifying the projection.
      Examples: "TC" (Transverse Cylindrical), "AC" (Albers Conic).

  - id: XOR
    type: str
    size: 15
    encoding: BCS-N
    doc: |
      X False Origin (XOR)
      15 BCS-N. False easting value for the projection origin.

  - id: YOR
    type: str
    size: 15
    encoding: BCS-N
    doc: |
      Y False Origin (YOR)
      15 BCS-N. False northing value for the projection origin.

  - id: PRN
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Number of Projection Parameters (PRN)
      1 BCS-N. Count of projection parameters (0-9).

  - id: PROJECTION_PARAMS
    type: projection_param
    repeat: expr
    repeat-expr: PRN.to_i
    doc: |
      Projection Parameters
      Repeated PRN times, each containing a parameter value and name.

types:
  projection_param:
    seq:
      - id: PCO
        type: str
        size: 15
        encoding: BCS-N
        doc: |
          Projection Parameter Value (PCO)
          15 BCS-N. Numeric value of the projection parameter.

      - id: PTB
        type: str
        size: 80
        encoding: ECS-A
        doc: |
          Projection Parameter Name (PTB)
          80 ECS-A. Name/description of the projection parameter.
