meta:
  id: nitf_02_10_text_subheader
  title: NITF 2.1 Text Subheader
  endian: be

doc: |
  NITF 2.1 (MIL-STD-2500C) Text Segment Subheader structure definition.
  This defines the text subheader portion of a NITF text segment,
  including text identification, security, and format information.

seq:
  # Text Segment Marker
  - id: TE
    type: str
    size: 2
    encoding: BCS-A
    doc: File part type. Always "TE" for text segments.

  # Text Identifier
  - id: TEXTID
    type: str
    size: 7
    encoding: BCS-A
    doc: Text identifier.

  # Text Attachment Level
  - id: TXTALVL
    type: str
    size: 3
    encoding: BCS-N
    doc: Text attachment level (000-998).

  # Text Date and Time
  - id: TXTDT
    type: str
    size: 14
    encoding: BCS-N
    doc: Text date and time (CCYYMMDDhhmmss format).

  # Text Title
  - id: TXTITL
    type: str
    size: 80
    encoding: ECS-A
    doc: Text title.

  # Text Security Classification
  - id: TSCLAS
    type: str
    size: 1
    encoding: BCS-A
    doc: Text security classification (T, S, C, R, or U).

  - id: TSCLSY
    type: str
    size: 2
    encoding: BCS-A
    doc: Text security classification system.

  - id: TSCODE
    type: str
    size: 11
    encoding: BCS-A
    doc: Text codewords.

  - id: TSCTLH
    type: str
    size: 2
    encoding: BCS-A
    doc: Text control and handling.

  - id: TSREL
    type: str
    size: 20
    encoding: BCS-A
    doc: Text releasing instructions.

  - id: TSDCTP
    type: str
    size: 2
    encoding: BCS-A
    doc: Text declassification type.

  - id: TSDCDT
    type: str
    size: 8
    encoding: BCS-N
    doc: Text declassification date.

  - id: TSDCXM
    type: str
    size: 4
    encoding: BCS-A
    doc: Text declassification exemption.

  - id: TSDG
    type: str
    size: 1
    encoding: BCS-A
    doc: Text downgrade.

  - id: TSDGDT
    type: str
    size: 8
    encoding: BCS-N
    doc: Text downgrade date.

  - id: TSCLTX
    type: str
    size: 43
    encoding: ECS-A
    doc: Text classification text.

  - id: TSCATP
    type: str
    size: 1
    encoding: BCS-A
    doc: Text classification authority type.

  - id: TSCAUT
    type: str
    size: 40
    encoding: ECS-A
    doc: Text classification authority.

  - id: TSCRSN
    type: str
    size: 1
    encoding: BCS-A
    doc: Text classification reason.

  - id: TSSRDT
    type: str
    size: 8
    encoding: BCS-N
    doc: Text security source date.

  - id: TSCTLN
    type: str
    size: 15
    encoding: BCS-A
    doc: Text security control number.

  # Encryption
  - id: ENCRYP
    type: str
    size: 1
    encoding: BCS-N
    doc: Encryption (0 = not encrypted).

  # Text Format
  - id: TXTFMT
    type: str
    size: 3
    encoding: BCS-A
    doc: Text format (MTF, STA, UT1, U8S).

  # Text Extended Subheader Data Length
  - id: TXSHDL
    type: str
    size: 5
    encoding: BCS-N
    doc: Text extended subheader data length.

  # Text Extended Subheader Overflow
  - id: TXSOFL
    type: str
    size: 3
    encoding: BCS-N
    if: TXSHDL.to_i > 0
    doc: Text extended subheader overflow.

  # Text Extended Subheader Data
  - id: TXSHD
    size: TXSHDL.to_i - 3
    if: TXSHDL.to_i > 0
    doc: Text extended subheader data (TREs).
