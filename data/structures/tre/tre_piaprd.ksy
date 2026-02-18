meta:
  id: tre_piaprd
  title: Profile for Imagery Access Product TRE
  endian: be

doc: |
  PIAPRD TRE - Profile for Imagery Access Product Support Extension - Version D
  
  Addresses information regarding products derived from source imagery.
  Aligns SPIA and NITF for product information with descriptive detail.
  Contains variable-length repeating fields for sections, organizations,
  keywords, reports, and text.
  
  Reference: STDI-0002 Volume 1, Appendix C - PIAE

seq:
  - id: accessid
    type: str
    size: 64
    encoding: ASCII
    doc: |
      Access ID (ACCESSID)
      Archive unique identifier (filename, record ID, reference number, etc.).
      64 BCS-A.

  - id: fmcontrol
    type: str
    size: 32
    encoding: ASCII
    doc: |
      FM Control Number (FMCONTROL)
      Foreign material associated with the product.
      32 BCS-A.

  - id: subdet
    type: str
    size: 1
    encoding: ASCII
    doc: |
      Subjective Detail (SUBDET)
      Subjective rating of useful detail available.
      1 BCS-A, P=Poor, F=Fair, G=Good, E=Excellent.

  - id: prodcode
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Product Code (PRODCODE)
      Category of product data stored in archive.
      2 BCS-A.

  - id: producerse
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Producer Supplement (PRODUCERSE)
      Element within producing organization that created product.
      6 BCS-A.

  - id: prodidno
    type: str
    size: 20
    encoding: ASCII
    doc: |
      Product ID Number (PRODIDNO)
      Producer assigned number identifying product.
      20 BCS-A.

  - id: prodsnme
    type: str
    size: 10
    encoding: ASCII
    doc: |
      Product Short Name (PRODSNME)
      Abbreviated name of product stored in archive.
      10 BCS-A. Required.

  - id: producercd
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Producer Code (PRODUCERCD)
      Organization responsible for creating/modifying product.
      2 BCS-A.

  - id: prodcrtime
    type: str
    size: 14
    encoding: ASCII
    doc: |
      Product Create Time (PRODCRTIME)
      Date/time product was created or last modified (ZULU).
      14 BCS-A, CCYYMMDDHHMMSS format.

  - id: mapid
    type: str
    size: 40
    encoding: ASCII
    doc: |
      Map ID (MAPID)
      Map associated with the product.
      40 BCS-A.

  - id: sectitlerep
    type: str
    size: 2
    encoding: ASCII
    doc: |
      SECTITLE Repetitions (SECTITLEREP)
      Number of times SECTITLE/PPNUM/TPP fields repeat.
      2 BCS-N, 00-99.

  - id: sectitle_entries
    type: sectitle_entry
    repeat: expr
    repeat-expr: sectitlerep.to_i
    doc: Section title entries (SECTITLE, PPNUM, TPP groups)

  - id: reqorgrep
    type: str
    size: 2
    encoding: ASCII
    doc: |
      REQORG Repetitions (REQORGREP)
      Number of times REQORG field repeats.
      2 BCS-N, 00-99.

  - id: reqorg_entries
    type: str
    size: 64
    encoding: ASCII
    repeat: expr
    repeat-expr: reqorgrep.to_i
    doc: Requesting Organization entries (64 BCS-A each)

  - id: keywordrep
    type: str
    size: 2
    encoding: ASCII
    doc: |
      KEYWORD Repetitions (KEYWORDREP)
      Number of times KEYWORD field repeats.
      2 BCS-N, 00-99.

  - id: keyword_entries
    type: str
    size: 255
    encoding: ASCII
    repeat: expr
    repeat-expr: keywordrep.to_i
    doc: Keyword string entries (255 BCS-A each)

  - id: assrptrep
    type: str
    size: 2
    encoding: ASCII
    doc: |
      ASSRPT Repetitions (ASSRPTREP)
      Number of times ASSRPT field repeats.
      2 BCS-N, 00-99.

  - id: assrpt_entries
    type: str
    size: 20
    encoding: ASCII
    repeat: expr
    repeat-expr: assrptrep.to_i
    doc: Associated Report entries (20 BCS-A each)

  - id: atextrep
    type: str
    size: 2
    encoding: ASCII
    doc: |
      ATEXT Repetitions (ATEXTREP)
      Number of times ATEXT field repeats.
      2 BCS-N, 00-99.

  - id: atext_entries
    type: str
    size: 255
    encoding: ASCII
    repeat: expr
    repeat-expr: atextrep.to_i
    doc: Associated Text entries (255 BCS-A each)

types:
  sectitle_entry:
    seq:
      - id: sectitle
        type: str
        size: 40
        encoding: ASCII
        doc: Section Title (40 BCS-A)
      - id: ppnum
        type: str
        size: 5
        encoding: ASCII
        doc: Page/Part Number (5 BCS-A)
      - id: tpp
        type: str
        size: 3
        encoding: ASCII
        doc: Total Pages/Parts (3 BCS-N, 001-999)
