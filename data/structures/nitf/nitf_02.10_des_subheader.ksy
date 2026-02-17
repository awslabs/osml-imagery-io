meta:
  id: nitf_02_10_des_subheader
  title: NITF 2.1 Data Extension Segment Subheader
  endian: be

doc: |
  NITF 2.1 (MIL-STD-2500C) Data Extension Segment (DES) Subheader structure definition.
  This defines the DES subheader portion of a NITF data extension segment,
  including DES identification, security, and overflow information.

seq:
  # DES Segment Marker
  - id: de
    type: str
    size: 2
    encoding: BCS-A
    doc: File part type. Always "DE" for data extension segments.

  # DES Unique Identifier
  - id: desid
    type: str
    size: 25
    encoding: BCS-A
    doc: Unique DES type identifier.

  # DES Version
  - id: desver
    type: str
    size: 2
    encoding: BCS-N
    doc: Version of the data definition (01-99).

  # DES Security Classification
  - id: desclas
    type: str
    size: 1
    encoding: BCS-A
    doc: DES security classification (T, S, C, R, or U).

  - id: desclsy
    type: str
    size: 2
    encoding: BCS-A
    doc: DES security classification system.

  - id: descode
    type: str
    size: 11
    encoding: BCS-A
    doc: DES codewords.

  - id: desctlh
    type: str
    size: 2
    encoding: BCS-A
    doc: DES control and handling.

  - id: desrel
    type: str
    size: 20
    encoding: BCS-A
    doc: DES releasing instructions.

  - id: desdctp
    type: str
    size: 2
    encoding: BCS-A
    doc: DES declassification type.

  - id: desdcdt
    type: str
    size: 8
    encoding: BCS-N
    doc: DES declassification date.

  - id: desdcxm
    type: str
    size: 4
    encoding: BCS-A
    doc: DES declassification exemption.

  - id: desdg
    type: str
    size: 1
    encoding: BCS-A
    doc: DES downgrade.

  - id: desdgdt
    type: str
    size: 8
    encoding: BCS-N
    doc: DES downgrade date.

  - id: descltx
    type: str
    size: 43
    encoding: ECS-A
    doc: DES classification text.

  - id: descatp
    type: str
    size: 1
    encoding: BCS-A
    doc: DES classification authority type.

  - id: descaut
    type: str
    size: 40
    encoding: ECS-A
    doc: DES classification authority.

  - id: descrsn
    type: str
    size: 1
    encoding: BCS-A
    doc: DES classification reason.

  - id: dessrdt
    type: str
    size: 8
    encoding: BCS-N
    doc: DES security source date.

  - id: desctln
    type: str
    size: 15
    encoding: BCS-A
    doc: DES security control number.

  # DES Overflowed Header Type (conditional on DESID)
  - id: desoflw
    type: str
    size: 6
    encoding: BCS-A
    if: desid == "TRE_OVERFLOW             "
    doc: Overflowed header type (UDHD, UDSHD, IXSHD, SXSHD, TXSHD).

  # DES Data Item Overflowed (conditional on DESID)
  - id: desitem
    type: str
    size: 3
    encoding: BCS-N
    if: desid == "TRE_OVERFLOW             "
    doc: Data item overflowed (segment index).

  # DES Defined Subheader Fields Length
  - id: desshl
    type: str
    size: 4
    encoding: BCS-N
    doc: Length of DES-defined subheader fields.

  # DES Defined Subheader Fields
  - id: desshf
    size: desshl.to_i
    if: desshl.to_i > 0
    doc: DES-defined subheader fields.
