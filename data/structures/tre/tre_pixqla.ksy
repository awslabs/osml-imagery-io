meta:
  id: tre_pixqla
  title: Pixel Quality TRE
  endian: be

doc: |
  PIXQLA TRE - Pixel Quality Tagged Record Extension
  
  Provides pixel quality metadata encoded within one or more NITF image
  segments that relate to pixel values recorded in one or more other image
  segments. The PIXQLA TRE describes the encoding of various pixel quality
  conditions within a Pixel Quality image Segment (PQS).
  
  The TRE defines:
  - Number of associated image segments (NUMAIS)
  - Display levels of associated image segments (AISDLVL)
  - Number of pixel quality conditions (NPIXQUAL)
  - Bit value indicating condition presence (PQ_BIT_VALUE)
  - Pixel quality condition names (PQ_CONDITION)
  
  Reference: STDI-0002 Volume 1, Appendix AA - PIXQLA

seq:
  - id: NUMAIS
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Number of Associated Image Segments (NUMAIS)
      Designates the number of image segments associated with the PQS.
      Value "ALL" means PQS is associated with all non-PIXQUAL image segments.
      Otherwise, a number from 001 to 998.
      3 BCS-A characters.

  - id: AISDLVL
    type: str
    size: 3
    encoding: BCS-N
    repeat: expr
    repeat-expr: NUMAIS.to_i
    if: NUMAIS != "ALL"
    doc: |
      Associated Image Segment Display Level (AISDLVL)
      Identifies the Image Display Level (IDLVL) of each image segment
      associated with the PQS. Repeated NUMAIS times.
      Omitted if NUMAIS = "ALL".
      3 BCS-N characters, range 001-999.

  - id: NPIXQUAL
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Number of Pixel Quality Conditions (NPIXQUAL)
      Designates the number of pixel quality conditions represented by
      the per-pixel values in the PQS. Equal to the number of bits used
      to encode pixel quality.
      4 BCS-N characters, range 0001-0064.

  - id: PQ_BIT_VALUE
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Pixel Quality Bit Value (PQ_BIT_VALUE)
      Identifies the value of the nth bit when PQ_CONDITIONn is present.
      1 BCS-A character, value "1".

  - id: PQ_CONDITION
    type: str
    size: 40
    encoding: BCS-A
    repeat: expr
    repeat-expr: NPIXQUAL.to_i
    doc: |
      Pixel Quality Condition (PQ_CONDITION)
      Identifies the pixel quality condition in the associated image segment
      when the nth bit of the corresponding pixel in the PQS is set to
      PQ_BIT_VALUE. Case-sensitive alphanumeric string.
      Common values: Bad, Saturated, Dead, Noisy, Vignetted, Fill, Gap,
      Blinker, Spurious-response, Interpolated, Averaged, Calibration-adjusted.
      40 BCS-A characters.
