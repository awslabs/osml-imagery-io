meta:
  id: nitf_02_10_file_header
  title: NITF 2.1 File Header
  endian: be
  file-extension: ntf

doc: |
  NITF 2.1 (MIL-STD-2500C) File Header structure definition.
  This defines the file header portion of a NITF file, including
  security metadata, segment counts, and segment length information.

seq:
  # File Profile and Version
  - id: fhdr
    type: str
    size: 4
    encoding: BCS-A
    doc: File profile name. "NITF" for NITF files, "NSIF" for NSIF files.

  - id: fver
    type: str
    size: 5
    encoding: BCS-A
    doc: File version. "02.10" for NITF 2.1.

  # Complexity Level and System Type
  - id: clevel
    type: str
    size: 2
    encoding: BCS-N
    doc: Complexity level (01-99). Indicates file complexity.

  - id: stype
    type: str
    size: 4
    encoding: BCS-A
    doc: Standard type. "BF01" for NITF 2.1.

  # Originating Station
  - id: ostaid
    type: str
    size: 10
    encoding: BCS-A
    doc: Originating station ID.

  # File Date and Time
  - id: fdt
    type: str
    size: 14
    encoding: BCS-N
    doc: File date and time (CCYYMMDDhhmmss format).

  # File Title
  - id: ftitle
    type: str
    size: 80
    encoding: ECS-A
    doc: File title.

  # File Security Classification
  - id: fsclas
    type: str
    size: 1
    encoding: BCS-A
    doc: File security classification (T, S, C, R, or U).

  - id: fsclsy
    type: str
    size: 2
    encoding: BCS-A
    doc: File security classification system.

  - id: fscode
    type: str
    size: 11
    encoding: BCS-A
    doc: File codewords.

  - id: fsctlh
    type: str
    size: 2
    encoding: BCS-A
    doc: File control and handling.

  - id: fsrel
    type: str
    size: 20
    encoding: BCS-A
    doc: File releasing instructions.

  - id: fsdctp
    type: str
    size: 2
    encoding: BCS-A
    doc: File declassification type.

  - id: fsdcdt
    type: str
    size: 8
    encoding: BCS-N
    doc: File declassification date.

  - id: fsdcxm
    type: str
    size: 4
    encoding: BCS-A
    doc: File declassification exemption.

  - id: fsdg
    type: str
    size: 1
    encoding: BCS-A
    doc: File downgrade.

  - id: fsdgdt
    type: str
    size: 8
    encoding: BCS-N
    doc: File downgrade date.

  - id: fscltx
    type: str
    size: 43
    encoding: ECS-A
    doc: File classification text.

  - id: fscatp
    type: str
    size: 1
    encoding: BCS-A
    doc: File classification authority type.

  - id: fscaut
    type: str
    size: 40
    encoding: ECS-A
    doc: File classification authority.

  - id: fscrsn
    type: str
    size: 1
    encoding: BCS-A
    doc: File classification reason.

  - id: fssrdt
    type: str
    size: 8
    encoding: BCS-N
    doc: File security source date.

  - id: fsctln
    type: str
    size: 15
    encoding: BCS-A
    doc: File security control number.

  # Copy and Version Numbers
  - id: fscop
    type: str
    size: 5
    encoding: BCS-N
    doc: File copy number.

  - id: fscpys
    type: str
    size: 5
    encoding: BCS-N
    doc: File number of copies.

  # Encryption
  - id: encryp
    type: str
    size: 1
    encoding: BCS-N
    doc: Encryption (0 = not encrypted).

  # Background Color (FBKGC) - 3 bytes binary
  - id: fbkgc
    size: 3
    doc: File background color (RGB).

  # Originator Information
  - id: oname
    type: str
    size: 24
    encoding: ECS-A
    doc: Originator's name.

  - id: ophone
    type: str
    size: 18
    encoding: BCS-A
    doc: Originator's phone number.

  # File Length
  - id: fl
    type: str
    size: 12
    encoding: BCS-N
    doc: File length in bytes.

  # Header Length
  - id: hl
    type: str
    size: 6
    encoding: BCS-N
    doc: NITF file header length.

  # Number of Image Segments
  - id: numi
    type: str
    size: 3
    encoding: BCS-N
    doc: Number of image segments (000-999).

  # Image Segment Info (repeated NUMI times)
  - id: image_info
    type: image_segment_info
    repeat: expr
    repeat-expr: numi.to_i
    doc: Image segment subheader and data lengths.

  # Number of Graphic Segments
  - id: nums
    type: str
    size: 3
    encoding: BCS-N
    doc: Number of graphic segments (000-999).

  # Graphic Segment Info (repeated NUMS times)
  - id: graphic_info
    type: graphic_segment_info
    repeat: expr
    repeat-expr: nums.to_i
    doc: Graphic segment subheader and data lengths.

  # Reserved for Future Use
  - id: numx
    type: str
    size: 3
    encoding: BCS-N
    doc: Reserved for future use.

  # Number of Text Segments
  - id: numt
    type: str
    size: 3
    encoding: BCS-N
    doc: Number of text segments (000-999).

  # Text Segment Info (repeated NUMT times)
  - id: text_info
    type: text_segment_info
    repeat: expr
    repeat-expr: numt.to_i
    doc: Text segment subheader and data lengths.

  # Number of Data Extension Segments
  - id: numdes
    type: str
    size: 3
    encoding: BCS-N
    doc: Number of data extension segments (000-999).

  # DES Info (repeated NUMDES times)
  - id: des_info
    type: des_segment_info
    repeat: expr
    repeat-expr: numdes.to_i
    doc: DES subheader and data lengths.

  # Number of Reserved Extension Segments
  - id: numres
    type: str
    size: 3
    encoding: BCS-N
    doc: Number of reserved extension segments (000-999).

  # RES Info (repeated NUMRES times)
  - id: res_info
    type: res_segment_info
    repeat: expr
    repeat-expr: numres.to_i
    doc: RES subheader and data lengths.

  # User Defined Header Data Length
  - id: udhdl
    type: str
    size: 5
    encoding: BCS-N
    doc: User defined header data length.

  # User Defined Header Overflow
  - id: udhofl
    type: str
    size: 3
    encoding: BCS-N
    if: udhdl.to_i > 0
    doc: User defined header overflow.

  # User Defined Header Data
  - id: udhd
    size: udhdl.to_i - 3
    if: udhdl.to_i > 0
    doc: User defined header data.

  # Extended Header Data Length
  - id: xhdl
    type: str
    size: 5
    encoding: BCS-N
    doc: Extended header data length.

  # Extended Header Overflow
  - id: xhdlofl
    type: str
    size: 3
    encoding: BCS-N
    if: xhdl.to_i > 0
    doc: Extended header data overflow.

  # Extended Header Data
  - id: xhd
    size: xhdl.to_i - 3
    if: xhdl.to_i > 0
    doc: Extended header data.

types:
  image_segment_info:
    doc: Image segment length information.
    seq:
      - id: lish
        type: str
        size: 6
        encoding: BCS-N
        doc: Length of image subheader.
      - id: li
        type: str
        size: 10
        encoding: BCS-N
        doc: Length of image data.

  graphic_segment_info:
    doc: Graphic segment length information.
    seq:
      - id: lssh
        type: str
        size: 4
        encoding: BCS-N
        doc: Length of graphic subheader.
      - id: ls
        type: str
        size: 6
        encoding: BCS-N
        doc: Length of graphic data.

  text_segment_info:
    doc: Text segment length information.
    seq:
      - id: ltsh
        type: str
        size: 4
        encoding: BCS-N
        doc: Length of text subheader.
      - id: lt
        type: str
        size: 5
        encoding: BCS-N
        doc: Length of text data.

  des_segment_info:
    doc: Data extension segment length information.
    seq:
      - id: ldsh
        type: str
        size: 4
        encoding: BCS-N
        doc: Length of DES subheader.
      - id: ld
        type: str
        size: 9
        encoding: BCS-N
        doc: Length of DES data.

  res_segment_info:
    doc: Reserved extension segment length information.
    seq:
      - id: lresh
        type: str
        size: 4
        encoding: BCS-N
        doc: Length of RES subheader.
      - id: lre
        type: str
        size: 7
        encoding: BCS-N
        doc: Length of RES data.
