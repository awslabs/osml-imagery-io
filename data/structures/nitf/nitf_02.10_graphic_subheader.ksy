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
  - id: sy
    type: str
    size: 2
    encoding: BCS-A
    doc: File part type. Always "SY" for graphic segments.

  # Graphic Identifier
  - id: sid
    type: str
    size: 10
    encoding: BCS-A
    doc: Graphic identifier.

  # Graphic Name
  - id: sname
    type: str
    size: 20
    encoding: ECS-A
    doc: Graphic name.

  # Graphic Security Classification
  - id: ssclas
    type: str
    size: 1
    encoding: BCS-A
    doc: Graphic security classification (T, S, C, R, or U).

  - id: ssclsy
    type: str
    size: 2
    encoding: BCS-A
    doc: Graphic security classification system.

  - id: sscode
    type: str
    size: 11
    encoding: BCS-A
    doc: Graphic codewords.

  - id: ssctlh
    type: str
    size: 2
    encoding: BCS-A
    doc: Graphic control and handling.

  - id: ssrel
    type: str
    size: 20
    encoding: BCS-A
    doc: Graphic releasing instructions.

  - id: ssdctp
    type: str
    size: 2
    encoding: BCS-A
    doc: Graphic declassification type.

  - id: ssdcdt
    type: str
    size: 8
    encoding: BCS-N
    doc: Graphic declassification date.

  - id: ssdcxm
    type: str
    size: 4
    encoding: BCS-A
    doc: Graphic declassification exemption.

  - id: ssdg
    type: str
    size: 1
    encoding: BCS-A
    doc: Graphic downgrade.

  - id: ssdgdt
    type: str
    size: 8
    encoding: BCS-N
    doc: Graphic downgrade date.

  - id: sscltx
    type: str
    size: 43
    encoding: ECS-A
    doc: Graphic classification text.

  - id: sscatp
    type: str
    size: 1
    encoding: BCS-A
    doc: Graphic classification authority type.

  - id: sscaut
    type: str
    size: 40
    encoding: ECS-A
    doc: Graphic classification authority.

  - id: sscrsn
    type: str
    size: 1
    encoding: BCS-A
    doc: Graphic classification reason.

  - id: sssrdt
    type: str
    size: 8
    encoding: BCS-N
    doc: Graphic security source date.

  - id: ssctln
    type: str
    size: 15
    encoding: BCS-A
    doc: Graphic security control number.

  # Encryption
  - id: encryp
    type: str
    size: 1
    encoding: BCS-N
    doc: Encryption (0 = not encrypted).

  # Graphic Type
  - id: sfmt
    type: str
    size: 1
    encoding: BCS-A
    doc: Graphic type (C = CGM).

  # Reserved for Future Use
  - id: sstruct
    type: str
    size: 13
    encoding: BCS-N
    doc: Reserved for future use.

  # Graphic Display Level
  - id: sdlvl
    type: str
    size: 3
    encoding: BCS-N
    doc: Graphic display level (001-999).

  # Graphic Attachment Level
  - id: salvl
    type: str
    size: 3
    encoding: BCS-N
    doc: Graphic attachment level (000-998).

  # Graphic Location
  - id: sloc
    type: str
    size: 10
    encoding: BCS-N
    doc: Graphic location (RRRRRCCCCC format).

  # First Graphic Bound Location
  - id: sbnd1
    type: str
    size: 10
    encoding: BCS-N
    doc: First graphic bound location.

  # Graphic Color
  - id: scolor
    type: str
    size: 1
    encoding: BCS-A
    doc: Graphic color (C = color, M = monochrome).

  # Second Graphic Bound Location
  - id: sbnd2
    type: str
    size: 10
    encoding: BCS-N
    doc: Second graphic bound location.

  # Reserved
  - id: sres2
    type: str
    size: 2
    encoding: BCS-N
    doc: Reserved for future use.

  # Graphic Extended Subheader Data Length
  - id: sxshdl
    type: str
    size: 5
    encoding: BCS-N
    doc: Graphic extended subheader data length.

  # Graphic Extended Subheader Overflow
  - id: sxsofl
    type: str
    size: 3
    encoding: BCS-N
    if: sxshdl.to_i > 0
    doc: Graphic extended subheader overflow.

  # Graphic Extended Subheader Data
  - id: sxshd
    size: sxshdl.to_i - 3
    if: sxshdl.to_i > 0
    doc: Graphic extended subheader data (TREs).
