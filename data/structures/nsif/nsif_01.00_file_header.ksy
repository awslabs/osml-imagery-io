meta:
  id: nsif_01_00_file_header
  title: NSIF 1.0 File Header
  endian: be
  file-extension: nsf

doc: |
  NSIF 1.0 (NATO Secondary Imagery Format) File Header structure definition.
  NSIF 1.0 is functionally equivalent to NITF 2.1 with different magic number.
  This defines the file header portion of an NSIF file, including
  security metadata, segment counts, and segment length information.

seq:
  # File Profile and Version
  - id: FHDR
    type: str
    size: 4
    encoding: BCS-A
    doc: File profile name. "NSIF" for NSIF files.

  - id: FVER
    type: str
    size: 5
    encoding: BCS-A
    doc: File version. "01.00" for NSIF 1.0.

  # Complexity Level and System Type
  - id: CLEVEL
    type: str
    size: 2
    encoding: BCS-N
    doc: Complexity level (01-99). Indicates file complexity.

  - id: STYPE
    type: str
    size: 4
    encoding: BCS-A
    doc: Standard type. "BF01" for NSIF 1.0.

  # Originating Station
  - id: OSTAID
    type: str
    size: 10
    encoding: BCS-A
    doc: Originating station ID.

  # File Date and Time
  - id: FDT
    type: str
    size: 14
    encoding: BCS-N
    doc: File date and time (CCYYMMDDhhmmss format).

  # File Title
  - id: FTITLE
    type: str
    size: 80
    encoding: ECS-A
    doc: File title.

  # File Security Classification
  - id: FSCLAS
    type: str
    size: 1
    encoding: BCS-A
    doc: File security classification (T, S, C, R, or U).

  - id: FSCLSY
    type: str
    size: 2
    encoding: BCS-A
    doc: File security classification system.

  - id: FSCODE
    type: str
    size: 11
    encoding: BCS-A
    doc: File codewords.

  - id: FSCTLH
    type: str
    size: 2
    encoding: BCS-A
    doc: File control and handling.

  - id: FSREL
    type: str
    size: 20
    encoding: BCS-A
    doc: File releasing instructions.

  - id: FSDCTP
    type: str
    size: 2
    encoding: BCS-A
    doc: File declassification type.

  - id: FSDCDT
    type: str
    size: 8
    encoding: BCS-N
    doc: File declassification date.

  - id: FSDCXM
    type: str
    size: 4
    encoding: BCS-A
    doc: File declassification exemption.

  - id: FSDG
    type: str
    size: 1
    encoding: BCS-A
    doc: File downgrade.

  - id: FSDGDT
    type: str
    size: 8
    encoding: BCS-N
    doc: File downgrade date.

  - id: FSCLTX
    type: str
    size: 43
    encoding: ECS-A
    doc: File classification text.

  - id: FSCATP
    type: str
    size: 1
    encoding: BCS-A
    doc: File classification authority type.

  - id: FSCAUT
    type: str
    size: 40
    encoding: ECS-A
    doc: File classification authority.

  - id: FSCRSN
    type: str
    size: 1
    encoding: BCS-A
    doc: File classification reason.

  - id: FSSRDT
    type: str
    size: 8
    encoding: BCS-N
    doc: File security source date.

  - id: FSCTLN
    type: str
    size: 15
    encoding: BCS-A
    doc: File security control number.

  # Copy and Version Numbers
  - id: FSCOP
    type: str
    size: 5
    encoding: BCS-N
    doc: File copy number.

  - id: FSCPYS
    type: str
    size: 5
    encoding: BCS-N
    doc: File number of copies.

  # Encryption
  - id: ENCRYP
    type: str
    size: 1
    encoding: BCS-N
    doc: Encryption (0 = not encrypted).

  # Background Color (FBKGC) - 3 bytes binary
  - id: FBKGC
    size: 3
    type: bytes
    doc: File background color (RGB).

  # Originator Information
  - id: ONAME
    type: str
    size: 24
    encoding: ECS-A
    doc: Originator's name.

  - id: OPHONE
    type: str
    size: 18
    encoding: BCS-A
    doc: Originator's phone number.

  # File Length
  - id: FL
    type: str
    size: 12
    encoding: BCS-N
    doc: File length in bytes.

  # Header Length
  - id: HL
    type: str
    size: 6
    encoding: BCS-N
    doc: NSIF file header length.

  # Number of Image Segments
  - id: NUMI
    type: str
    size: 3
    encoding: BCS-N
    doc: Number of image segments (000-999).

  # Image Segment Info (repeated NUMI times)
  - id: IMAGE_INFO
    type: image_segment_info
    repeat: expr
    repeat-expr: NUMI.to_i
    doc: Image segment subheader and data lengths.

  # Number of Graphic Segments
  - id: NUMS
    type: str
    size: 3
    encoding: BCS-N
    doc: Number of graphic segments (000-999).

  # Graphic Segment Info (repeated NUMS times)
  - id: GRAPHIC_INFO
    type: graphic_segment_info
    repeat: expr
    repeat-expr: NUMS.to_i
    doc: Graphic segment subheader and data lengths.

  # Reserved for Future Use
  - id: NUMX
    type: str
    size: 3
    encoding: BCS-N
    doc: Reserved for future use.

  # Number of Text Segments
  - id: NUMT
    type: str
    size: 3
    encoding: BCS-N
    doc: Number of text segments (000-999).

  # Text Segment Info (repeated NUMT times)
  - id: TEXT_INFO
    type: text_segment_info
    repeat: expr
    repeat-expr: NUMT.to_i
    doc: Text segment subheader and data lengths.

  # Number of Data Extension Segments
  - id: NUMDES
    type: str
    size: 3
    encoding: BCS-N
    doc: Number of data extension segments (000-999).

  # DES Info (repeated NUMDES times)
  - id: DES_INFO
    type: des_segment_info
    repeat: expr
    repeat-expr: NUMDES.to_i
    doc: DES subheader and data lengths.

  # Number of Reserved Extension Segments
  - id: NUMRES
    type: str
    size: 3
    encoding: BCS-N
    doc: Number of reserved extension segments (000-999).

  # RES Info (repeated NUMRES times)
  - id: RES_INFO
    type: res_segment_info
    repeat: expr
    repeat-expr: NUMRES.to_i
    doc: RES subheader and data lengths.

  # User Defined Header Data Length
  - id: UDHDL
    type: str
    size: 5
    encoding: BCS-N
    doc: User defined header data length.

  # User Defined Header Overflow
  - id: UDHOFL
    type: str
    size: 3
    encoding: BCS-N
    if: UDHDL.to_i > 0
    doc: User defined header overflow.

  # User Defined Header Data
  - id: UDHD
    size: UDHDL.to_i - 3
    if: UDHDL.to_i > 0
    doc: User defined header data.

  # Extended Header Data Length
  - id: XHDL
    type: str
    size: 5
    encoding: BCS-N
    doc: Extended header data length.

  # Extended Header Overflow
  - id: XHDLOFL
    type: str
    size: 3
    encoding: BCS-N
    if: XHDL.to_i > 0
    doc: Extended header data overflow.

  # Extended Header Data
  - id: XHD
    size: XHDL.to_i - 3
    if: XHDL.to_i > 0
    doc: Extended header data.

types:
  image_segment_info:
    doc: Image segment length information.
    seq:
      - id: LISH
        type: str
        size: 6
        encoding: BCS-N
        doc: Length of image subheader.
      - id: LI
        type: str
        size: 10
        encoding: BCS-N
        doc: Length of image data.

  graphic_segment_info:
    doc: Graphic segment length information.
    seq:
      - id: LSSH
        type: str
        size: 4
        encoding: BCS-N
        doc: Length of graphic subheader.
      - id: LS
        type: str
        size: 6
        encoding: BCS-N
        doc: Length of graphic data.

  text_segment_info:
    doc: Text segment length information.
    seq:
      - id: LTSH
        type: str
        size: 4
        encoding: BCS-N
        doc: Length of text subheader.
      - id: LT
        type: str
        size: 5
        encoding: BCS-N
        doc: Length of text data.

  des_segment_info:
    doc: Data extension segment length information.
    seq:
      - id: LDSH
        type: str
        size: 4
        encoding: BCS-N
        doc: Length of DES subheader.
      - id: LD
        type: str
        size: 9
        encoding: BCS-N
        doc: Length of DES data.

  res_segment_info:
    doc: Reserved extension segment length information.
    seq:
      - id: LRESH
        type: str
        size: 4
        encoding: BCS-N
        doc: Length of RES subheader.
      - id: LRE
        type: str
        size: 7
        encoding: BCS-N
        doc: Length of RES data.
