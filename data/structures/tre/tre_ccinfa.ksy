meta:
  id: tre_ccinfa
  title: Country Code Information TRE
  endian: be

doc: |
  CCINFA TRE - Country Code Information Tagged Record Extension
  
  Provides translations from legacy country codes (a priori or ad hoc) to
  GENC standard-defined short URN-based individual item identifiers.
  
  Supports three types of translations:
  - A priori translations: Standard code to GENC URN
  - Ad hoc translations: Custom code to GENC URN
  - Ad hoc clarifications: More specific translation for a priori code
  
  Each translation can optionally include XML metadata with country name
  details, either uncompressed or gzip-compressed.
  
  Reference: STDI-0002 Volume 1, Appendix AG - CCINFA

seq:
  - id: NUMCODE
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Number of Defined Codes (NUMCODE)
      Number of code translations defined in this TRE instance.
      3 BCS-N characters, range 1-999.

  - id: CODES
    type: code_entry
    repeat: expr
    repeat-expr: NUMCODE.to_i
    doc: |
      Code translation entries.
      Repeated NUMCODE times.

types:
  code_entry:
    seq:
      - id: CODE_LEN
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Length of the CODE field (CODE_LEN)
          1 BCS-N character, range 1-9.

      - id: CODE
        type: str
        size: CODE_LEN.to_i
        encoding: BCS-A
        doc: |
          Code (CODE)
          A priori or ad hoc country code.
          1-9 BCS-A characters.

      - id: EQTYPE
        type: str
        size: 1
        encoding: BCS-A
        doc: |
          Type of Equivalence (EQTYPE)
          Space = completely equivalent, "C" = ad hoc clarification.
          1 BCS-A character.

      - id: ESURN_LEN
        type: str
        size: 2
        encoding: BCS-N
        doc: |
          Length of the ESURN field (ESURN_LEN)
          2 BCS-N characters, range 9-99.

      - id: ESURN
        type: str
        size: ESURN_LEN.to_i
        encoding: BCS-A
        doc: |
          Equivalent Short URN-based Individual Item Identifier (ESURN)
          Valid short URN-based individual item identifier per GENC standard.
          Example: ge:GENC:3:3-5:USA
          9-99 BCS-A characters.

      - id: DETAIL_LEN
        type: str
        size: 5
        encoding: BCS-N
        doc: |
          Length of the DETAIL field (DETAIL_LEN)
          5 BCS-N characters, range 0 to max remaining data.

      - id: DETAIL_CMPR
        type: str
        size: 1
        encoding: BCS-A
        if: DETAIL_LEN.to_i > 0
        doc: |
          Code Detail Compression (DETAIL_CMPR)
          Space = uncompressed XML, "G" = gzip compressed.
          1 BCS-A character.
          Only present if DETAIL_LEN > 0.

      - id: DETAIL
        size: DETAIL_LEN.to_i
        if: DETAIL_LEN.to_i > 0
        doc: |
          Code Detail (DETAIL)
          XML metadata per GENC standard schema, optionally gzip compressed.
          Variable length.
          Only present if DETAIL_LEN > 0.
