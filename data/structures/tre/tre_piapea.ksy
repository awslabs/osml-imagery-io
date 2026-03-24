meta:
  id: tre_piapea
  title: Profile for Imagery Access Person TRE
  endian: be

doc: |
  PIAPEA TRE - Profile for Imagery Access Person
  
  Provides person identification information associated with
  imagery products, including name and country of association.
  
  Fixed length: 92 bytes.
  
  Derived from GDAL nitf_spec.xml definition (2026-03-24):
  https://github.com/OSGeo/gdal/blob/master/frmts/nitf/data/nitf_spec.xml
  
  Reference: STDI-0002 Volume 1, Appendix C - PIAE

seq:
  - id: LASTNME
    type: str
    size: 28
    encoding: BCS-A
    doc: |
      Last Name
      28 BCS-A.

  - id: FIRSTNME
    type: str
    size: 28
    encoding: BCS-A
    doc: |
      First Name
      28 BCS-A.

  - id: MIDNME
    type: str
    size: 28
    encoding: BCS-A
    doc: |
      Middle Name
      28 BCS-A.

  - id: DOB
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      Date of Birth
      6 BCS-A.

  - id: ASSOCTRY
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Associated Country
      2 BCS-A.
