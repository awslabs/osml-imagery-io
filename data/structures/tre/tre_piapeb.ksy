meta:
  id: tre_piapeb
  title: Profile for Imagery Access Person TRE
  endian: be

doc: |
  PIAPEB TRE - Profile for Imagery Access Person Identification Extension - Version B
  
  Identifies information related to person(s) contained in imagery.
  Present for each person identified, up to 500 occurrences per data type.
  
  Reference: STDI-0002 Volume 1, Appendix C - PIAE

seq:
  - id: lastnme
    type: str
    size: 28
    encoding: ASCII
    doc: |
      Last Name (LASTNME)
      Surname of individual captured in image.
      28 BCS-A.

  - id: firstnme
    type: str
    size: 28
    encoding: ASCII
    doc: |
      First Name (FIRSTNME)
      First name of individual captured in image.
      28 BCS-A.

  - id: midnme
    type: str
    size: 28
    encoding: ASCII
    doc: |
      Middle Name (MIDNME)
      Middle name of individual captured in image.
      28 BCS-A.

  - id: dob
    type: str
    size: 8
    encoding: ASCII
    doc: |
      Date of Birth (DOB)
      Birth date of individual captured in image.
      8 BCS-A, CCMMDDYY format.

  - id: assoctry
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Associated Country (ASSOCTRY)
      Country the person is associated with.
      2 BCS-A, GEC code.
