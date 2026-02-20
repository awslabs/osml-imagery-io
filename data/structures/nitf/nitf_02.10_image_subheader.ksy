meta:
  id: nitf_02_10_image_subheader
  title: NITF 2.1 Image Subheader
  endian: be

doc: |
  NITF 2.1 (MIL-STD-2500C) Image Segment Subheader structure definition.
  This defines the image subheader portion of a NITF image segment,
  including image identification, security, dimensions, and compression info.

seq:
  # Image Segment Marker
  - id: IM
    type: str
    size: 2
    encoding: BCS-A
    doc: File part type. Always "IM" for image segments.

  # Image Identifiers
  - id: IID1
    type: str
    size: 10
    encoding: BCS-A
    doc: Image identifier 1.

  - id: IDATIM
    type: str
    size: 14
    encoding: BCS-N
    doc: Image date and time (CCYYMMDDhhmmss format).

  - id: TGTID
    type: str
    size: 17
    encoding: BCS-A
    doc: Target identifier.

  - id: IID2
    type: str
    size: 80
    encoding: ECS-A
    doc: Image identifier 2 (free text).

  # Image Security Classification
  - id: ISCLAS
    type: str
    size: 1
    encoding: BCS-A
    doc: Image security classification (T, S, C, R, or U).

  - id: ISCLSY
    type: str
    size: 2
    encoding: BCS-A
    doc: Image security classification system.

  - id: ISCODE
    type: str
    size: 11
    encoding: BCS-A
    doc: Image codewords.

  - id: ISCTLH
    type: str
    size: 2
    encoding: BCS-A
    doc: Image control and handling.

  - id: ISREL
    type: str
    size: 20
    encoding: BCS-A
    doc: Image releasing instructions.

  - id: ISDCTP
    type: str
    size: 2
    encoding: BCS-A
    doc: Image declassification type.

  - id: ISDCDT
    type: str
    size: 8
    encoding: BCS-N
    doc: Image declassification date.

  - id: ISDCXM
    type: str
    size: 4
    encoding: BCS-A
    doc: Image declassification exemption.

  - id: ISDG
    type: str
    size: 1
    encoding: BCS-A
    doc: Image downgrade.

  - id: ISDGDT
    type: str
    size: 8
    encoding: BCS-N
    doc: Image downgrade date.

  - id: ISCLTX
    type: str
    size: 43
    encoding: ECS-A
    doc: Image classification text.

  - id: ISCATP
    type: str
    size: 1
    encoding: BCS-A
    doc: Image classification authority type.

  - id: ISCAUT
    type: str
    size: 40
    encoding: ECS-A
    doc: Image classification authority.

  - id: ISCRSN
    type: str
    size: 1
    encoding: BCS-A
    doc: Image classification reason.

  - id: ISSRDT
    type: str
    size: 8
    encoding: BCS-N
    doc: Image security source date.

  - id: ISCTLN
    type: str
    size: 15
    encoding: BCS-A
    doc: Image security control number.

  # Encryption
  - id: ENCRYP
    type: str
    size: 1
    encoding: BCS-N
    doc: Encryption (0 = not encrypted).

  # Image Source
  - id: ISORCE
    type: str
    size: 42
    encoding: ECS-A
    doc: Image source.

  # Image Dimensions
  - id: NROWS
    type: str
    size: 8
    encoding: BCS-N
    doc: Number of significant rows in image.

  - id: NCOLS
    type: str
    size: 8
    encoding: BCS-N
    doc: Number of significant columns in image.

  # Pixel Value Type
  - id: PVTYPE
    type: str
    size: 3
    encoding: BCS-A
    doc: Pixel value type (INT, B, SI, R, C).

  # Image Representation
  - id: IREP
    type: str
    size: 8
    encoding: BCS-A
    doc: Image representation (MONO, RGB, RGB/LUT, MULTI, NODISPLY, NVECTOR, POLAR, VPH, YCbCr601).

  # Image Category
  - id: ICAT
    type: str
    size: 8
    encoding: BCS-A
    doc: Image category.

  # Actual Bits Per Pixel
  - id: ABPP
    type: str
    size: 2
    encoding: BCS-N
    doc: Actual bits per pixel per band.

  # Pixel Justification
  - id: PJUST
    type: str
    size: 1
    encoding: BCS-A
    doc: Pixel justification (R = right, L = left).

  # Image Coordinate Representation
  - id: ICORDS
    type: str
    size: 1
    encoding: BCS-A
    doc: Image coordinate representation (U, N, S, G, D, blank).

  # Image Geographic Location
  - id: IGEOLO
    type: str
    size: 60
    encoding: BCS-A
    if: ICORDS != " " and ICORDS != ""
    doc: Image geographic location (4 corner coordinates).

  # Number of Image Comments
  - id: NICOM
    type: str
    size: 1
    encoding: BCS-N
    doc: Number of image comments (0-9).

  # Image Comments
  - id: ICOM
    type: str
    size: 80
    encoding: ECS-A
    repeat: expr
    repeat-expr: NICOM.to_i
    doc: Image comment.

  # Image Compression
  - id: IC
    type: str
    size: 2
    encoding: BCS-A
    doc: Image compression (NC, NM, C1, C3, C4, C5, C6, C7, C8, I1, M1, M3, M4, M5, M6, M7, M8).


  # Compression Rate Code (conditional)
  - id: COMRAT
    type: str
    size: 4
    encoding: BCS-A
    if: IC != "NC" and IC != "NM"
    doc: Compression rate code.

  # Number of Bands
  - id: NBANDS
    type: str
    size: 1
    encoding: BCS-N
    doc: Number of bands (0-9, 0 means use XBANDS).

  # Extended Number of Bands
  - id: XBANDS
    type: str
    size: 5
    encoding: BCS-N
    if: NBANDS.to_i == 0
    doc: Number of bands when NBANDS=0.

  # Band Info (repeated for each band) - when NBANDS > 0
  - id: BAND_INFO
    type: band_info_type
    repeat: expr
    repeat-expr: NBANDS.to_i
    if: NBANDS.to_i > 0
    doc: Band information for each band (when NBANDS > 0).

  # Band Info (repeated for each band) - when NBANDS == 0, use XBANDS
  - id: BAND_INFO_EXTENDED
    type: band_info_type
    repeat: expr
    repeat-expr: XBANDS.to_i
    if: NBANDS.to_i == 0
    doc: Band information for each band (when NBANDS == 0, using XBANDS).

  # Image Sync Code
  - id: ISYNC
    type: str
    size: 1
    encoding: BCS-N
    doc: Image sync code (0 = no sync).

  # Image Mode
  - id: IMODE
    type: str
    size: 1
    encoding: BCS-A
    doc: Image mode (B, P, R, S).

  # Number of Blocks Per Row
  - id: NBPR
    type: str
    size: 4
    encoding: BCS-N
    doc: Number of blocks per row.

  # Number of Blocks Per Column
  - id: NBPC
    type: str
    size: 4
    encoding: BCS-N
    doc: Number of blocks per column.

  # Number of Pixels Per Block Horizontal
  - id: NPPBH
    type: str
    size: 4
    encoding: BCS-N
    doc: Number of pixels per block horizontal.

  # Number of Pixels Per Block Vertical
  - id: NPPBV
    type: str
    size: 4
    encoding: BCS-N
    doc: Number of pixels per block vertical.

  # Number of Bits Per Pixel
  - id: NBPP
    type: str
    size: 2
    encoding: BCS-N
    doc: Number of bits per pixel per band.

  # Image Display Level
  - id: IDLVL
    type: str
    size: 3
    encoding: BCS-N
    doc: Image display level (001-999).

  # Image Attachment Level
  - id: IALVL
    type: str
    size: 3
    encoding: BCS-N
    doc: Image attachment level (000-998).

  # Image Location
  - id: ILOC
    type: str
    size: 10
    encoding: BCS-N
    doc: Image location (RRRRRCCCCC format).

  # Image Magnification
  - id: IMAG
    type: str
    size: 4
    encoding: BCS-A
    doc: Image magnification.

  # User Defined Image Data Length
  - id: UDIDL
    type: str
    size: 5
    encoding: BCS-N
    doc: User defined image data length.

  # User Defined Overflow
  - id: UDOFL
    type: str
    size: 3
    encoding: BCS-N
    if: UDIDL.to_i > 0
    doc: User defined overflow.

  # User Defined Image Data
  - id: UDID
    size: UDIDL.to_i - 3
    if: UDIDL.to_i > 0
    doc: User defined image data.

  # Image Extended Subheader Data Length
  - id: IXSHDL
    type: str
    size: 5
    encoding: BCS-N
    doc: Image extended subheader data length.

  # Image Extended Subheader Overflow
  - id: IXSOFL
    type: str
    size: 3
    encoding: BCS-N
    if: IXSHDL.to_i > 0
    doc: Image extended subheader overflow.

  # Image Extended Subheader Data
  - id: IXSHD
    size: IXSHDL.to_i - 3
    if: IXSHDL.to_i > 0
    doc: Image extended subheader data (TREs).

types:
  band_info_type:
    doc: Band information for a single band.
    seq:
      - id: IREPBAND
        type: str
        size: 2
        encoding: BCS-A
        doc: Band representation (R, G, B, M, LU, Y, Cb, Cr, blank).

      - id: ISUBCAT
        type: str
        size: 6
        encoding: BCS-A
        doc: Band subcategory.

      - id: IFC
        type: str
        size: 1
        encoding: BCS-A
        doc: Band image filter condition (N = none).

      - id: IMFLT
        type: str
        size: 3
        encoding: BCS-A
        doc: Band standard image filter code.

      - id: NLUTS
        type: str
        size: 1
        encoding: BCS-N
        doc: Number of LUTs for this band (0-4).

      - id: NELUT
        type: str
        size: 5
        encoding: BCS-N
        if: NLUTS.to_i > 0
        doc: Number of entries in each LUT.

      - id: LUT_DATA
        size: NELUT.to_i
        repeat: expr
        repeat-expr: NLUTS.to_i
        if: NLUTS.to_i > 0
        doc: LUT data.
