meta:
  id: tre_piaprc
  title: Profile for Imagery Access Product TRE (Version C)
  endian: be

doc: |
  PIAPRC TRE - Profile for Imagery Access Product Support Extension - Version C
  
  Addresses information regarding products derived from source imagery.
  Structurally identical to PIAPRD but designated as Version C.
  Contains variable-length repeating fields for sections, organizations,
  keywords, reports, and text.
  
  Derived from GDAL nitf_spec.xml definition (2026-03-24):
  https://github.com/OSGeo/gdal/blob/master/frmts/nitf/data/nitf_spec.xml
  
  Reference: STDI-0002 Volume 1, Appendix C - PIAE

seq:
  - id: ACCESSID
    type: str
    size: 64
    encoding: BCS-A
    doc: |
      Access ID
      64 BCS-A.

  - id: FMCONTROL
    type: str
    size: 32
    encoding: BCS-A
    doc: |
      FM Control Number
      32 BCS-A.

  - id: SUBDET
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Subjective Detail
      1 BCS-A.

  - id: PRODCODE
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Product Code
      2 BCS-A.

  - id: PRODUCERSE
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      Producer Supplement
      6 BCS-A.

  - id: PRODIDNO
    type: str
    size: 20
    encoding: BCS-A
    doc: |
      Product ID Number
      20 BCS-A.

  - id: PRODSNME
    type: str
    size: 10
    encoding: BCS-A
    doc: |
      Product Short Name
      10 BCS-A.

  - id: PRODUCERCD
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Producer Code
      2 BCS-A.

  - id: PRODCRTIME
    type: str
    size: 14
    encoding: BCS-A
    doc: |
      Product Create Time
      14 BCS-A, CCYYMMDDHHMMSS format.

  - id: MAPID
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      Map ID
      40 BCS-A.

  - id: SECTITLEREP
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      SECTITLE Repetitions
      2 BCS-N integer, range 00-99.

  - id: SECTITLE_ENTRIES
    type: sectitle_entry
    repeat: expr
    repeat-expr: SECTITLEREP.to_i
    doc: Section title entries.

  - id: REQORGREP
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      REQORG Repetitions
      2 BCS-N integer, range 00-99.

  - id: REQORG_ENTRIES
    type: str
    size: 64
    encoding: BCS-A
    repeat: expr
    repeat-expr: REQORGREP.to_i
    doc: Requesting Organization entries (64 BCS-A each).

  - id: KEYWORDREP
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      KEYWORD Repetitions
      2 BCS-N integer, range 00-99.

  - id: KEYWORD_ENTRIES
    type: str
    size: 255
    encoding: BCS-A
    repeat: expr
    repeat-expr: KEYWORDREP.to_i
    doc: Keyword entries (255 BCS-A each).

  - id: ASSRPTREP
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      ASSRPT Repetitions
      2 BCS-N integer, range 00-99.

  - id: ASSRPT_ENTRIES
    type: str
    size: 20
    encoding: BCS-A
    repeat: expr
    repeat-expr: ASSRPTREP.to_i
    doc: Associated Report entries (20 BCS-A each).

  - id: ATEXTREP
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      ATEXT Repetitions
      2 BCS-N integer, range 00-99.

  - id: ATEXT_ENTRIES
    type: str
    size: 255
    encoding: BCS-A
    repeat: expr
    repeat-expr: ATEXTREP.to_i
    doc: Associated Text entries (255 BCS-A each).

types:
  sectitle_entry:
    seq:
      - id: SECTITLE
        type: str
        size: 40
        encoding: BCS-A
        doc: Section Title (40 BCS-A).
      - id: PPNUM
        type: str
        size: 5
        encoding: BCS-A
        doc: Page/Part Number (5 BCS-A).
      - id: TPP
        type: str
        size: 3
        encoding: BCS-N
        doc: Total Pages/Parts (3 BCS-N, range 001-999).
