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
  - id: im
    type: str
    size: 2
    encoding: BCS-A
    doc: File part type. Always "IM" for image segments.

  # Image Identifiers
  - id: iid1
    type: str
    size: 10
    encoding: BCS-A
    doc: Image identifier 1.

  - id: idatim
    type: str
    size: 14
    encoding: BCS-N
    doc: Image date and time (CCYYMMDDhhmmss format).

  - id: tgtid
    type: str
    size: 17
    encoding: BCS-A
    doc: Target identifier.

  - id: iid2
    type: str
    size: 80
    encoding: ECS-A
    doc: Image identifier 2 (free text).

  # Image Security Classification
  - id: isclas
    type: str
    size: 1
    encoding: BCS-A
    doc: Image security classification (T, S, C, R, or U).

  - id: isclsy
    type: str
    size: 2
    encoding: BCS-A
    doc: Image security classification system.

  - id: iscode
    type: str
    size: 11
    encoding: BCS-A
    doc: Image codewords.

  - id: isctlh
    type: str
    size: 2
    encoding: BCS-A
    doc: Image control and handling.

  - id: isrel
    type: str
    size: 20
    encoding: BCS-A
    doc: Image releasing instructions.

  - id: isdctp
    type: str
    size: 2
    encoding: BCS-A
    doc: Image declassification type.

  - id: isdcdt
    type: str
    size: 8
    encoding: BCS-N
    doc: Image declassification date.

  - id: isdcxm
    type: str
    size: 4
    encoding: BCS-A
    doc: Image declassification exemption.

  - id: isdg
    type: str
    size: 1
    encoding: BCS-A
    doc: Image downgrade.

  - id: isdgdt
    type: str
    size: 8
    encoding: BCS-N
    doc: Image downgrade date.

  - id: iscltx
    type: str
    size: 43
    encoding: ECS-A
    doc: Image classification text.

  - id: iscatp
    type: str
    size: 1
    encoding: BCS-A
    doc: Image classification authority type.

  - id: iscaut
    type: str
    size: 40
    encoding: ECS-A
    doc: Image classification authority.

  - id: iscrsn
    type: str
    size: 1
    encoding: BCS-A
    doc: Image classification reason.

  - id: issrdt
    type: str
    size: 8
    encoding: BCS-N
    doc: Image security source date.

  - id: isctln
    type: str
    size: 15
    encoding: BCS-A
    doc: Image security control number.

  # Encryption
  - id: encryp
    type: str
    size: 1
    encoding: BCS-N
    doc: Encryption (0 = not encrypted).

  # Image Source
  - id: isorce
    type: str
    size: 42
    encoding: ECS-A
    doc: Image source.

  # Image Dimensions
  - id: nrows
    type: str
    size: 8
    encoding: BCS-N
    doc: Number of significant rows in image.

  - id: ncols
    type: str
    size: 8
    encoding: BCS-N
    doc: Number of significant columns in image.

  # Pixel Value Type
  - id: pvtype
    type: str
    size: 3
    encoding: BCS-A
    doc: Pixel value type (INT, B, SI, R, C).

  # Image Representation
  - id: irep
    type: str
    size: 8
    encoding: BCS-A
    doc: Image representation (MONO, RGB, RGB/LUT, MULTI, NODISPLY, NVECTOR, POLAR, VPH, YCbCr601).

  # Image Category
  - id: icat
    type: str
    size: 8
    encoding: BCS-A
    doc: Image category.

  # Actual Bits Per Pixel
  - id: abpp
    type: str
    size: 2
    encoding: BCS-N
    doc: Actual bits per pixel per band.

  # Pixel Justification
  - id: pjust
    type: str
    size: 1
    encoding: BCS-A
    doc: Pixel justification (R = right, L = left).

  # Image Coordinate Representation
  - id: icords
    type: str
    size: 1
    encoding: BCS-A
    doc: Image coordinate representation (U, N, S, G, D, blank).

  # Image Geographic Location
  - id: igeolo
    type: str
    size: 60
    encoding: BCS-A
    if: icords != " " and icords != ""
    doc: Image geographic location (4 corner coordinates).

  # Number of Image Comments
  - id: nicom
    type: str
    size: 1
    encoding: BCS-N
    doc: Number of image comments (0-9).

  # Image Comments
  - id: icom
    type: str
    size: 80
    encoding: ECS-A
    repeat: expr
    repeat-expr: nicom.to_i
    doc: Image comment.

  # Image Compression
  - id: ic
    type: str
    size: 2
    encoding: BCS-A
    doc: Image compression (NC, NM, C1, C3, C4, C5, C6, C7, C8, I1, M1, M3, M4, M5, M6, M7, M8).


  # Compression Rate Code (conditional)
  - id: comrat
    type: str
    size: 4
    encoding: BCS-A
    if: ic != "NC" and ic != "NM"
    doc: Compression rate code.

  # Number of Bands
  - id: nbands
    type: str
    size: 1
    encoding: BCS-N
    doc: Number of bands (0-9, 0 means use XBANDS).

  # Extended Number of Bands
  - id: xbands
    type: str
    size: 5
    encoding: BCS-N
    if: nbands.to_i == 0
    doc: Number of bands when NBANDS=0.

  # Band Info (repeated for each band) - when NBANDS > 0
  - id: band_info
    type: band_info_type
    repeat: expr
    repeat-expr: nbands.to_i
    if: nbands.to_i > 0
    doc: Band information for each band (when NBANDS > 0).

  # Band Info (repeated for each band) - when NBANDS == 0, use XBANDS
  - id: band_info_extended
    type: band_info_type
    repeat: expr
    repeat-expr: xbands.to_i
    if: nbands.to_i == 0
    doc: Band information for each band (when NBANDS == 0, using XBANDS).

  # Image Sync Code
  - id: isync
    type: str
    size: 1
    encoding: BCS-N
    doc: Image sync code (0 = no sync).

  # Image Mode
  - id: imode
    type: str
    size: 1
    encoding: BCS-A
    doc: Image mode (B, P, R, S).

  # Number of Blocks Per Row
  - id: nbpr
    type: str
    size: 4
    encoding: BCS-N
    doc: Number of blocks per row.

  # Number of Blocks Per Column
  - id: nbpc
    type: str
    size: 4
    encoding: BCS-N
    doc: Number of blocks per column.

  # Number of Pixels Per Block Horizontal
  - id: nppbh
    type: str
    size: 4
    encoding: BCS-N
    doc: Number of pixels per block horizontal.

  # Number of Pixels Per Block Vertical
  - id: nppbv
    type: str
    size: 4
    encoding: BCS-N
    doc: Number of pixels per block vertical.

  # Number of Bits Per Pixel
  - id: nbpp
    type: str
    size: 2
    encoding: BCS-N
    doc: Number of bits per pixel per band.

  # Image Display Level
  - id: idlvl
    type: str
    size: 3
    encoding: BCS-N
    doc: Image display level (001-999).

  # Image Attachment Level
  - id: ialvl
    type: str
    size: 3
    encoding: BCS-N
    doc: Image attachment level (000-998).

  # Image Location
  - id: iloc
    type: str
    size: 10
    encoding: BCS-N
    doc: Image location (RRRRRCCCCC format).

  # Image Magnification
  - id: imag
    type: str
    size: 4
    encoding: BCS-A
    doc: Image magnification.

  # User Defined Image Data Length
  - id: udidl
    type: str
    size: 5
    encoding: BCS-N
    doc: User defined image data length.

  # User Defined Overflow
  - id: udofl
    type: str
    size: 3
    encoding: BCS-N
    if: udidl.to_i > 0
    doc: User defined overflow.

  # User Defined Image Data
  - id: udid
    size: udidl.to_i - 3
    if: udidl.to_i > 0
    doc: User defined image data.

  # Image Extended Subheader Data Length
  - id: ixshdl
    type: str
    size: 5
    encoding: BCS-N
    doc: Image extended subheader data length.

  # Image Extended Subheader Overflow
  - id: ixsofl
    type: str
    size: 3
    encoding: BCS-N
    if: ixshdl.to_i > 0
    doc: Image extended subheader overflow.

  # Image Extended Subheader Data
  - id: ixshd
    size: ixshdl.to_i - 3
    if: ixshdl.to_i > 0
    doc: Image extended subheader data (TREs).

types:
  band_info_type:
    doc: Band information for a single band.
    seq:
      - id: irepband
        type: str
        size: 2
        encoding: BCS-A
        doc: Band representation (R, G, B, M, LU, Y, Cb, Cr, blank).

      - id: isubcat
        type: str
        size: 6
        encoding: BCS-A
        doc: Band subcategory.

      - id: ifc
        type: str
        size: 1
        encoding: BCS-A
        doc: Band image filter condition (N = none).

      - id: imflt
        type: str
        size: 3
        encoding: BCS-A
        doc: Band standard image filter code.

      - id: nluts
        type: str
        size: 1
        encoding: BCS-N
        doc: Number of LUTs for this band (0-4).

      - id: nelut
        type: str
        size: 5
        encoding: BCS-N
        if: nluts.to_i > 0
        doc: Number of entries in each LUT.

      - id: lut_data
        size: nelut.to_i
        repeat: expr
        repeat-expr: nluts.to_i
        if: nluts.to_i > 0
        doc: LUT data.
