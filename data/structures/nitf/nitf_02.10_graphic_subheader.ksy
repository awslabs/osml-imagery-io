meta:
  id: nitf_02_10_graphic_subheader
  title: NITF 2.1 Graphic Subheader
  endian: be

doc: |
  NITF 2.1 (MIL-STD-2500C) Graphic Segment Subheader structure definition.
  This defines the graphic subheader portion of a NITF graphic segment,
  including graphic identification, security, and display information.

seq:
  # Graphic Segment Marker
  - id: SY
    type: str
    size: 2
    encoding: BCS-A
    doc: File part type. Always "SY" for graphic segments.

  # Graphic Identifier
  - id: SID
    type: str
    size: 10
    encoding: BCS-A
    doc: Graphic identifier.

  # Graphic Name
  - id: SNAME
    type: str
    size: 20
    encoding: ECS-A
    doc: Graphic name.

  # Graphic Security Classification
  - id: SSCLAS
    type: str
    size: 1
    encoding: BCS-A
    doc: Graphic security classification (T, S, C, R, or U).

  - id: SSCLSY
    type: str
    size: 2
    encoding: BCS-A
    doc: Graphic security classification system.

  - id: SSCODE
    type: str
    size: 11
    encoding: BCS-A
    doc: Graphic codewords.

  - id: SSCTLH
    type: str
    size: 2
    encoding: BCS-A
    doc: Graphic control and handling.

  - id: SSREL
    type: str
    size: 20
    encoding: BCS-A
    doc: Graphic releasing instructions.

  - id: SSDCTP
    type: str
    size: 2
    encoding: BCS-A
    doc: Graphic declassification type.

  - id: SSDCDT
    type: str
    size: 8
    encoding: BCS-N
    doc: Graphic declassification date.

  - id: SSDCXM
    type: str
    size: 4
    encoding: BCS-A
    doc: Graphic declassification exemption.

  - id: SSDG
    type: str
    size: 1
    encoding: BCS-A
    doc: Graphic downgrade.

  - id: SSDGDT
    type: str
    size: 8
    encoding: BCS-N
    doc: Graphic downgrade date.

  - id: SSCLTX
    type: str
    size: 43
    encoding: ECS-A
    doc: Graphic classification text.

  - id: SSCATP
    type: str
    size: 1
    encoding: BCS-A
    doc: Graphic classification authority type.

  - id: SSCAUT
    type: str
    size: 40
    encoding: ECS-A
    doc: Graphic classification authority.

  - id: SSCRSN
    type: str
    size: 1
    encoding: BCS-A
    doc: Graphic classification reason.

  - id: SSSRDT
    type: str
    size: 8
    encoding: BCS-N
    doc: Graphic security source date.

  - id: SSCTLN
    type: str
    size: 15
    encoding: BCS-A
    doc: Graphic security control number.

  # Encryption
  - id: ENCRYP
    type: str
    size: 1
    encoding: BCS-N
    doc: Encryption (0 = not encrypted).

  # Graphic Type
  - id: SFMT
    type: str
    size: 1
    encoding: BCS-A
    doc: Graphic type (C = CGM).

  # Reserved for Future Use
  - id: SSTRUCT
    type: str
    size: 13
    encoding: BCS-N
    doc: Reserved for future use.

  # Graphic Display Level
  - id: SDLVL
    type: str
    size: 3
    encoding: BCS-N
    doc: Graphic display level (001-999).

  # Graphic Attachment Level
  - id: SALVL
    type: str
    size: 3
    encoding: BCS-N
    doc: Graphic attachment level (000-998).

  # Graphic Location
  - id: SLOC
    type: str
    size: 10
    encoding: BCS-N
    doc: Graphic location (RRRRRCCCCC format).

  # First Graphic Bound Location
  - id: SBND1
    type: str
    size: 10
    encoding: BCS-N
    doc: First graphic bound location.

  # Graphic Color
  - id: SCOLOR
    type: str
    size: 1
    encoding: BCS-A
    doc: Graphic color (C = color, M = monochrome).

  # Second Graphic Bound Location
  - id: SBND2
    type: str
    size: 10
    encoding: BCS-N
    doc: Second graphic bound location.

  # Reserved
  - id: SRES2
    type: str
    size: 2
    encoding: BCS-N
    doc: Reserved for future use.

  # Graphic Extended Subheader Data Length
  - id: SXSHDL
    type: str
    size: 5
    encoding: BCS-N
    doc: Graphic extended subheader data length.

  # Graphic Extended Subheader Overflow
  - id: SXSOFL
    type: str
    size: 3
    encoding: BCS-N
    if: SXSHDL.to_i > 0
    doc: Graphic extended subheader overflow.

  # Graphic Extended Subheader Data
  - id: SXSHD
    size: SXSHDL.to_i - 3
    if: SXSHDL.to_i > 0
    doc: Graphic extended subheader data (TREs).
