meta:
  id: tre_histoa
  title: Softcopy History TRE
  endian: be

doc: |
  HISTOA TRE - Softcopy History Tagged Record Extension
  
  Describes previous pixel processing actions and the current state of the
  image pixels. Records processing events including compression, rotation,
  sharpening, magnification, dynamic range adjustment, and tonal transfer
  curves applied to the imagery.
  
  The TRE structure allows recording up to 99 separate processing events.
  Each processing event contains fields indicating the type of processing
  applied at that moment in time.
  
  Reference: STDI-0002 Volume 1, Appendix L - HISTOA

seq:
  - id: SYSTYPE
    type: str
    size: 20
    encoding: BCS-A
    doc: |
      System Type - Name of the sensor from which the original image was
      collected. Left justified, space padded to 20 characters.
      Examples: ALIRT, ASARS-2, BUCKEY, GHR, GORGON STARE, etc.

  - id: PC
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Prior Compression - Indicates if bandwidth compression/expansion was
      applied to the image prior to NITF image creation. 12 bytes allowing
      concatenation of up to 3 compression algorithms (4 bytes each).
      Examples: DP43, DC13, DC23, NJNL, JP20, J2NL, NONE, UNKC

  - id: PE
    type: str
    size: 4
    encoding: BCS-A
    doc: |
      Prior Enhancements - Indicates if any enhancements were applied to
      the image prior to NITF image creation.
      Values: EH08, EH11, UE08, UE11, DGHC, UNKP, NONE, GEOR, ORTH

  - id: REMAP_FLAG
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      System Specific Remap - Indicates if system specific remap was applied.
      0 = no remap, 1 = remap applied, space = not applicable,
      2-9 reserved for future use.

  - id: LUTID
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Data Mapping ID (DMID) from the ESD.
      00 = neither Linlog nor PEDF, 07-08 = PEDF, 11-64 = Linlog.
      01-06, 09-10 are reserved.

  - id: NEVENTS
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Number of Processing Events - Number of processing events associated
      with the image. Range: 01 to 99.

  - id: EVENTS
    type: processing_event
    repeat: expr
    repeat-expr: NEVENTS.to_i
    doc: Processing events in chronological order.

types:
  processing_event:
    doc: |
      A processing event records one or more specific processing functions
      applied to the NITF formatted image at a point in time.
    seq:
      - id: PDATE
        type: str
        size: 14
        encoding: BCS-N
        doc: |
          Processing Date and Time (UTC) - CCYYMMDDhhmmss format.

      - id: PSITE
        type: str
        size: 10
        encoding: BCS-A
        doc: |
          Processing Site - Name of site that performed the processing.
          Free form text. Examples: FOS, JWAC, CENTCOM.

      - id: PAS
        type: str
        size: 10
        encoding: BCS-A
        doc: |
          Processing Application Software - Software used to perform
          the processing steps. Examples: IDEX, VITEC, DIEPS.

      - id: NIPCOM
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Number of Image Processing Comments - Range: 0 to 9.

      - id: IPCOMS
        type: str
        size: 80
        encoding: BCS-A
        repeat: expr
        repeat-expr: NIPCOM.to_i
        doc: |
          Image Processing Comments - Free form text lines (80 chars each).
          Used to clarify or indicate special processing.

      - id: IBPP
        type: str
        size: 2
        encoding: BCS-N
        doc: |
          Input Bit Depth (actual) - Number of significant bits per pixel
          before processing. Range: 01 to 64.

      - id: IPVTYPE
        type: str
        size: 3
        encoding: BCS-A
        doc: |
          Input Pixel Value Type - Computer representation type.
          INT = integer, SI = signed integer, R = real, C = complex,
          B = bi-level, U = user defined.

      - id: INBWC
        type: str
        size: 10
        encoding: BCS-A
        doc: |
          Input Bandwidth Compression - Type of compression/expansion
          applied prior to enhancements. 10 bytes for up to 2 algorithms.
          Format: 4-char type + 1-char operation (C/E/0) repeated twice.

      - id: DISP_FLAG
        type: str
        size: 1
        encoding: BCS-A
        doc: |
          Display-Ready Flag - Indicates if image is display-ready.
          0 = not display-ready, 1 = display-ready, 2 = display-ready (no TTC),
          3 = display-ready (TTC allowed), space = inherently displayable.

      - id: ROT_FLAG
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Image Rotation Flag - 0 = not rotated, 1 = rotated.

      - id: ROT_ANGLE
        type: str
        size: 8
        encoding: BCS-N
        if: ROT_FLAG == "1"
        doc: |
          Angle of Rotation - Degrees clockwise. Range: 000.0000 to 359.9999.
          Floating decimal point permitted.

      - id: ASYM_FLAG
        type: str
        size: 1
        encoding: BCS-A
        doc: |
          Asymmetric Correction Flag - 0 = not applied, 1 = applied,
          space = not needed.

      - id: ZOOMROW
        type: str
        size: 7
        encoding: BCS-N
        if: ASYM_FLAG == "1"
        doc: |
          Magnification in Line (row) Direction - Range: 00.0000 to 99.9999.
          Floating decimal point permitted.

      - id: ZOOMCOL
        type: str
        size: 7
        encoding: BCS-N
        if: ASYM_FLAG == "1"
        doc: |
          Magnification in Element (column) Direction - Range: 00.0000 to 99.9999.
          Floating decimal point permitted.

      - id: PROJ_FLAG
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Image Projection Flag - 0 = no projection, 1 = projected.

      - id: SHARP_FLAG
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Sharpening Flag - 0 = no sharpening, 1 = sharpening applied.

      - id: SHARPFAM
        type: str
        size: 2
        encoding: BCS-A
        if: SHARP_FLAG == "1"
        doc: |
          Sharpening Family Number - Range: -1, 00 to 99.
          -1 indicates non-standard kernel (described in comments).

      - id: SHARPMEM
        type: str
        size: 2
        encoding: BCS-A
        if: SHARP_FLAG == "1"
        doc: |
          Sharpening Member Number - Range: -1, 00 to 99.
          -1 indicates non-standard kernel (described in comments).

      - id: MAG_FLAG
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Symmetrical Magnification Flag - 0 = not magnified, 1 = magnified.

      - id: MAG_LEVEL
        type: str
        size: 7
        encoding: BCS-N
        if: MAG_FLAG == "1"
        doc: |
          Level of Relative Magnification - Range: 00.0000 to 99.9999.
          Floating decimal point permitted. >1 = enlarged, <1 = reduced.

      - id: DRA_FLAG
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Dynamic Range Adjustment Flag - 0 = no DRA, 1 = spatially invariant DRA,
          2 = spatially variant DRA.

      - id: DRA_MULT
        type: str
        size: 7
        encoding: BCS-N
        if: DRA_FLAG == "1"
        doc: |
          DRA Multiplier - Range: 000.000 to 999.999.
          Floating decimal point permitted.

      - id: DRA_SUB
        type: str
        size: 5
        encoding: BCS-A
        if: DRA_FLAG == "1"
        doc: |
          DRA Subtractor - Range: -9999 to +9999.

      - id: TTC_FLAG
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Tonal Transfer Curve Flag - 0 = no TTC, 1 = TTC applied.

      - id: TTCFAM
        type: str
        size: 2
        encoding: BCS-A
        if: TTC_FLAG == "1"
        doc: |
          TTC Family Number - Range: -1, 00 to 99.
          -1 indicates non-standard TTC (described in comments).

      - id: TTCMEM
        type: str
        size: 2
        encoding: BCS-A
        if: TTC_FLAG == "1"
        doc: |
          TTC Member Number - Range: -1, 00 to 99.
          -1 indicates non-standard TTC (described in comments).

      - id: DEVLUT_FLAG
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Device LUT Flag - 0 = no device LUT, 1 = device LUT applied.

      - id: OBPP
        type: str
        size: 2
        encoding: BCS-N
        doc: |
          Output Bit Depth (actual) - Number of significant bits per pixel
          after processing. Range: 01 to 64.

      - id: OPVTYPE
        type: str
        size: 3
        encoding: BCS-A
        doc: |
          Output Pixel Value Type - Computer representation type.
          INT = integer, SI = signed integer, R = real, C = complex,
          B = bi-level, U = user defined.

      - id: OUTBWC
        type: str
        size: 10
        encoding: BCS-A
        doc: |
          Output Bandwidth Compression - Type of compression/expansion
          applied after enhancements. 10 bytes for up to 2 algorithms.
          Format: 4-char type + 1-char operation (C/E/0) repeated twice.
