meta:
  id: tre_imasda
  title: Image Scaling Data TRE
  endian: be

doc: |
  IMASDA TRE - Image Scaling Data
  
  Provides image scaling and translation parameters for DPPDB
  (Digital Point Positioning Data Base) products. Contains
  geographic and image coordinate transformation values.
  
  Fixed length: 242 bytes.
  
  Derived from GDAL nitf_spec.xml definition (2026-03-24):
  https://github.com/OSGeo/gdal/blob/master/frmts/nitf/data/nitf_spec.xml
  
  Reference: Table 68 of DPPDB specification (MIL-STD-89034)

seq:
  - id: LONTR
    type: str
    size: 22
    encoding: BCS-N
    doc: |
      Longitude Translation
      22 BCS-N real, degrees, range -180.0 to 180.0.

  - id: LATTR
    type: str
    size: 22
    encoding: BCS-N
    doc: |
      Latitude Translation
      22 BCS-N real, degrees, range -90.0 to 90.0.

  - id: ELVTR
    type: str
    size: 22
    encoding: BCS-N
    doc: |
      Elevation Translation
      22 BCS-N real, meters, range -1000.0 to 10000.0.

  - id: LONSC
    type: str
    size: 22
    encoding: BCS-N
    doc: |
      Longitude Scale
      22 BCS-N real, range 0.0 to 100.0.

  - id: LATSC
    type: str
    size: 22
    encoding: BCS-N
    doc: |
      Latitude Scale
      22 BCS-N real, range 0.0 to 100.0.

  - id: ELVSC
    type: str
    size: 22
    encoding: BCS-N
    doc: |
      Elevation Scale
      22 BCS-N real, range 0.0 to 100.0.

  - id: XITR
    type: str
    size: 22
    encoding: BCS-N
    doc: |
      X Image Translation
      22 BCS-N real, pixels, range -10000.0 to 10000.0.

  - id: YITR
    type: str
    size: 22
    encoding: BCS-N
    doc: |
      Y Image Translation
      22 BCS-N real, pixels, range -10000.0 to 10000.0.

  - id: XISC
    type: str
    size: 22
    encoding: BCS-N
    doc: |
      X Image Scale
      22 BCS-N real, range 0.0 to 100.0.

  - id: YISC
    type: str
    size: 22
    encoding: BCS-N
    doc: |
      Y Image Scale
      22 BCS-N real, range 0.0 to 100.0.

  - id: DELEV
    type: str
    size: 22
    encoding: BCS-N
    doc: |
      Default Elevation
      22 BCS-N real, meters, range -1000.0 to 10000.0.
