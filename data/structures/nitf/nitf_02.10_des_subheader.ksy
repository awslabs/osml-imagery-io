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
  - id: DE
    type: str
    size: 2
    encoding: BCS-A
    doc: File part type. Always "DE" for data extension segments.

  # DES Unique Identifier
  - id: DESID
    type: str
    size: 25
    encoding: BCS-A
    doc: Unique DES type identifier.

  # DES Version
  - id: DESVER
    type: str
    size: 2
    encoding: BCS-N
    doc: Version of the data definition (01-99).

  # DES Security Classification
  - id: DESCLAS
    type: str
    size: 1
    encoding: BCS-A
    doc: DES security classification (T, S, C, R, or U).

  - id: DESCLSY
    type: str
    size: 2
    encoding: BCS-A
    doc: DES security classification system.

  - id: DESCODE
    type: str
    size: 11
    encoding: BCS-A
    doc: DES codewords.

  - id: DESCTLH
    type: str
    size: 2
    encoding: BCS-A
    doc: DES control and handling.

  - id: DESREL
    type: str
    size: 20
    encoding: BCS-A
    doc: DES releasing instructions.

  - id: DESDCTP
    type: str
    size: 2
    encoding: BCS-A
    doc: DES declassification type.

  - id: DESDCDT
    type: str
    size: 8
    encoding: BCS-N
    doc: DES declassification date.

  - id: DESDCXM
    type: str
    size: 4
    encoding: BCS-A
    doc: DES declassification exemption.

  - id: DESDG
    type: str
    size: 1
    encoding: BCS-A
    doc: DES downgrade.

  - id: DESDGDT
    type: str
    size: 8
    encoding: BCS-N
    doc: DES downgrade date.

  - id: DESCLTX
    type: str
    size: 43
    encoding: ECS-A
    doc: DES classification text.

  - id: DESCATP
    type: str
    size: 1
    encoding: BCS-A
    doc: DES classification authority type.

  - id: DESCAUT
    type: str
    size: 40
    encoding: ECS-A
    doc: DES classification authority.

  - id: DESCRSN
    type: str
    size: 1
    encoding: BCS-A
    doc: DES classification reason.

  - id: DESSRDT
    type: str
    size: 8
    encoding: BCS-N
    doc: DES security source date.

  - id: DESCTLN
    type: str
    size: 15
    encoding: BCS-A
    doc: DES security control number.

  # DES Overflowed Header Type (conditional on DESID)
  - id: DESOFLW
    type: str
    size: 6
    encoding: BCS-A
    if: DESID == "TRE_OVERFLOW             "
    doc: Overflowed header type (UDHD, UDSHD, IXSHD, SXSHD, TXSHD).

  # DES Data Item Overflowed (conditional on DESID)
  - id: DESITEM
    type: str
    size: 3
    encoding: BCS-N
    if: DESID == "TRE_OVERFLOW             "
    doc: Data item overflowed (segment index).

  # DES Defined Subheader Fields Length
  - id: DESSHL
    type: str
    size: 4
    encoding: BCS-N
    doc: Length of DES-defined subheader fields.

  # DES Defined Subheader Fields
  - id: DESSHF
    size: DESSHL.to_i
    if: DESSHL.to_i > 0
    doc: DES-defined subheader fields.
