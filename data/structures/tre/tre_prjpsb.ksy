meta:
  id: tre_prjpsb
  title: Projection Parameters TRE
  endian: be

doc: |
  PRJPSB TRE - Projection Parameters Tagged Record Extension

  Provides map projection parameters for NITF images including
  the projection name, projection code, projection-specific
  parameters, and the projection false origins.

  Reference: STDI-0002 Volume 1, Appendix P - GEOSDE, Table P-3
  (pp. P-26 to P-28).

seq:
  - id: PRN
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Projection Name (PRN)
      3 BCS-A. Name of the projection to which the image segment
      refers (see Table P-20). Default value is "Transverse Mercator".

  - id: PCO
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Projection Code (PCO)
      2 BCS-A. Code of the projection to which the image segment
      refers (see Table P-20). Default value is "TC".

  - id: NUM_PRJ
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Number of Projection Parameters (NUM_PRJ)
      1 BCS-N. Count of projection parameters (0 to 9). The PRJn
      field is repeated NUM_PRJ times.

  - id: PROJECTION_PARAMS
    type: projection_param
    repeat: expr
    repeat-expr: NUM_PRJ.to_i
    doc: |
      Projection Parameters loop.
      Repeated NUM_PRJ times (only present when NUM_PRJ > 0).

  - id: XOR
    type: str
    size: 15
    encoding: BCS-N
    doc: |
      Projection False X (Easting) Origin (XOR)
      15 BCS-N. False easting value for the projection origin.
      Default 000000000000000 implies no false X origin.

  - id: YOR
    type: str
    size: 15
    encoding: BCS-N
    doc: |
      Projection False Y (Northing) Origin (YOR)
      15 BCS-N. False northing value for the projection origin.
      Default 000000000000000 implies no false Y origin.

types:
  projection_param:
    seq:
      - id: PRJ
        type: str
        size: 15
        encoding: BCS-N
        doc: |
          nth Projection Parameter (PRJn)
          15 BCS-N. An appropriate parameter value to accurately
          describe the projection (see Table P-20).
