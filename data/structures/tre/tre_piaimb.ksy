meta:
  id: tre_piaimb
  title: Profile for Imagery Access TRE (Version B)
  endian: be

doc: |
  PIAIMB TRE - Profile for Imagery Access Image Support Extension - Version B
  
  Provides imagery access profile information including cloud cover,
  sensor mode, sensor name, source, and other image characteristics.
  
  Fixed length: 337 bytes.
  
  Derived from GDAL nitf_spec.xml definition (2026-03-24):
  https://github.com/OSGeo/gdal/blob/master/frmts/nitf/data/nitf_spec.xml
  
  Reference: STDI-0002 Volume 1, Appendix C - PIAE

seq:
  - id: CLOUDCVR
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Cloud Cover
      3 BCS-A.

  - id: SRP
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Standard Radiometric Product
      1 BCS-A.

  - id: SENSMODE
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Sensor Mode
      12 BCS-A.

  - id: SENSNAME
    type: str
    size: 18
    encoding: BCS-A
    doc: |
      Sensor Name
      18 BCS-A.

  - id: SOURCE
    type: str
    size: 255
    encoding: BCS-A
    doc: |
      Source
      255 BCS-A.

  - id: COMGEN
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Compression Generation
      2 BCS-A.

  - id: SUBQUAL
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Subjective Quality
      1 BCS-A.

  - id: PIAMSNNUM
    type: str
    size: 7
    encoding: BCS-A
    doc: |
      PIA Mission Number
      7 BCS-A.

  - id: CAMSPECS
    type: str
    size: 32
    encoding: BCS-A
    doc: |
      Camera Specs
      32 BCS-A.

  - id: PROJID
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Project ID
      2 BCS-A.

  - id: GENERATION
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Generation
      1 BCS-A.

  - id: ESD
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Exploitation Support Data
      1 BCS-A.

  - id: OTHERCOND
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Other Conditions
      2 BCS-A.
