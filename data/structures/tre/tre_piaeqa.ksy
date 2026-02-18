meta:
  id: tre_piaeqa
  title: Profile for Imagery Access Equipment TRE
  endian: be

doc: |
  PIAEQA TRE - Profile for Imagery Access Equipment Extension - Version A
  
  Provides data related to equipment contained in an image.
  Present for each instance of equipment identified, up to 250 per data type.
  
  Reference: STDI-0002 Volume 1, Appendix C - PIAE

seq:
  - id: eqpcode
    type: str
    size: 7
    encoding: ASCII
    doc: |
      Equipment Code (EQPCODE)
      Equipment code from NGIC Foreign Equipment Guide.
      7 BCS-A.

  - id: eqpnomen
    type: str
    size: 45
    encoding: ASCII
    doc: |
      Equipment Nomenclature (EQPNOMEN)
      Equipment nomenclature from NGIC Foreign Equipment Guide.
      45 BCS-A.

  - id: eqpman
    type: str
    size: 64
    encoding: ASCII
    doc: |
      Equipment Manufacturer (EQPMAN)
      Manufacturer of the equipment.
      64 BCS-A.

  - id: obtype
    type: str
    size: 1
    encoding: ASCII
    doc: |
      OB Type (OBTYPE)
      Order of Battle type from MIIDS/IDB.
      1 BCS-A.

  - id: ordbat
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Type Order of Battle (ORDBAT)
      Type order of battle from EARS 1.1.
      3 BCS-A.

  - id: ctryprod
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Country Produced (CTRYPROD)
      Country where equipment was produced.
      2 BCS-A, GEC code.

  - id: ctrydsn
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Country Code Designed (CTRYDSN)
      Country where equipment was designed.
      2 BCS-A, GEC code.

  - id: objview
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Object View (OBJVIEW)
      View of the object in the image.
      6 BCS-A, Right/Left/Top/Bottom/Front/Rear.
