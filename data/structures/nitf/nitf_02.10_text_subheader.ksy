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
  - id: te
    type: str
    size: 2
    encoding: BCS-A
    doc: File part type. Always "TE" for text segments.

  # Text Identifier
  - id: textid
    type: str
    size: 7
    encoding: BCS-A
    doc: Text identifier.

  # Text Attachment Level
  - id: txtalvl
    type: str
    size: 3
    encoding: BCS-N
    doc: Text attachment level (000-998).

  # Text Date and Time
  - id: txtdt
    type: str
    size: 14
    encoding: BCS-N
    doc: Text date and time (CCYYMMDDhhmmss format).

  # Text Title
  - id: txtitl
    type: str
    size: 80
    encoding: ECS-A
    doc: Text title.

  # Text Security Classification
  - id: tsclas
    type: str
    size: 1
    encoding: BCS-A
    doc: Text security classification (T, S, C, R, or U).

  - id: tsclsy
    type: str
    size: 2
    encoding: BCS-A
    doc: Text security classification system.

  - id: tscode
    type: str
    size: 11
    encoding: BCS-A
    doc: Text codewords.

  - id: tsctlh
    type: str
    size: 2
    encoding: BCS-A
    doc: Text control and handling.

  - id: tsrel
    type: str
    size: 20
    encoding: BCS-A
    doc: Text releasing instructions.

  - id: tsdctp
    type: str
    size: 2
    encoding: BCS-A
    doc: Text declassification type.

  - id: tsdcdt
    type: str
    size: 8
    encoding: BCS-N
    doc: Text declassification date.

  - id: tsdcxm
    type: str
    size: 4
    encoding: BCS-A
    doc: Text declassification exemption.

  - id: tsdg
    type: str
    size: 1
    encoding: BCS-A
    doc: Text downgrade.

  - id: tsdgdt
    type: str
    size: 8
    encoding: BCS-N
    doc: Text downgrade date.

  - id: tscltx
    type: str
    size: 43
    encoding: ECS-A
    doc: Text classification text.

  - id: tscatp
    type: str
    size: 1
    encoding: BCS-A
    doc: Text classification authority type.

  - id: tscaut
    type: str
    size: 40
    encoding: ECS-A
    doc: Text classification authority.

  - id: tscrsn
    type: str
    size: 1
    encoding: BCS-A
    doc: Text classification reason.

  - id: tssrdt
    type: str
    size: 8
    encoding: BCS-N
    doc: Text security source date.

  - id: tsctln
    type: str
    size: 15
    encoding: BCS-A
    doc: Text security control number.

  # Encryption
  - id: encryp
    type: str
    size: 1
    encoding: BCS-N
    doc: Encryption (0 = not encrypted).

  # Text Format
  - id: txtfmt
    type: str
    size: 3
    encoding: BCS-A
    doc: Text format (MTF, STA, UT1, U8S).

  # Text Extended Subheader Data Length
  - id: txshdl
    type: str
    size: 5
    encoding: BCS-N
    doc: Text extended subheader data length.

  # Text Extended Subheader Overflow
  - id: txsofl
    type: str
    size: 3
    encoding: BCS-N
    if: txshdl.to_i > 0
    doc: Text extended subheader overflow.

  # Text Extended Subheader Data
  - id: txshd
    size: txshdl.to_i - 3
    if: txshdl.to_i > 0
    doc: Text extended subheader data (TREs).
