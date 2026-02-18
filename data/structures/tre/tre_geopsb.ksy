meta:
  id: tre_geopsb
  title: Geographic/Projected Coordinate System Parameters TRE
  endian: be

doc: |
  GEOPSB TRE - Geographic and Projected Coordinate System Parameters
  
  Provides coordinate system parameters including geodetic datum,
  ellipsoid, vertical datum, and grid information for NITF images.
  
  Reference: STDI-0002 Volume 1, Appendix P - GEOSDE

seq:
  - id: typ
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Coordinate System Type (TYP)
      3 BCS-A. Values: "GEO" (Geographic), "MAP" (Map Projected),
      "DIG" (DIGEST), "NA " (Not Applicable).

  - id: uni
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Unit of Measure (UNI)
      3 BCS-A. Values: "DEG" (Degrees), "M  " (Meters),
      "F  " (Feet), "NA " (Not Applicable).

  - id: dag
    type: str
    size: 80
    encoding: ECS-A
    doc: |
      Geodetic Datum Name (DAG)
      80 ECS-A. Full name of the geodetic datum.

  - id: dcd
    type: str
    size: 4
    encoding: BCS-A
    doc: |
      Geodetic Datum Code (DCD)
      4 BCS-A. Code identifying the geodetic datum.

  - id: ell
    type: str
    size: 80
    encoding: ECS-A
    doc: |
      Ellipsoid Name (ELL)
      80 ECS-A. Full name of the reference ellipsoid.

  - id: elc
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Ellipsoid Code (ELC)
      3 BCS-A. Code identifying the reference ellipsoid.

  - id: dvr
    type: str
    size: 80
    encoding: ECS-A
    doc: |
      Vertical Datum Reference (DVR)
      80 ECS-A. Name of the vertical datum reference.

  - id: vdcdvr
    type: str
    size: 4
    encoding: BCS-A
    doc: |
      Vertical Datum Code for DVR (VDCDVR)
      4 BCS-A. Code for the vertical datum reference.

  - id: sda
    type: str
    size: 80
    encoding: ECS-A
    doc: |
      Sounding Datum (SDA)
      80 ECS-A. Name of the sounding datum.

  - id: vdcsda
    type: str
    size: 4
    encoding: BCS-A
    doc: |
      Vertical Datum Code for SDA (VDCSDA)
      4 BCS-A. Code for the sounding datum.

  - id: zor
    type: str
    size: 15
    encoding: BCS-N
    doc: |
      Z False Origin (ZOR)
      15 BCS-N. False origin for Z (elevation) values.

  - id: grd
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Grid Code (GRD)
      3 BCS-A. Code identifying the grid system.

  - id: grn
    type: str
    size: 80
    encoding: ECS-A
    doc: |
      Grid Description (GRN)
      80 ECS-A. Full description of the grid system.

  - id: zna
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Zone Number (ZNA)
      4 BCS-N. Zone number for the grid system.
