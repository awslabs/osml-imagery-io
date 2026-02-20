meta:
  id: tre_ichipb
  title: Image Chip TRE
  endian: be

doc: |
  ICHIPB TRE - Image Chip Support Data Extension
  
  Provides image chip corner point coordinate information mapped to the
  original image coordinate system. Used for mensuration and geopositioning
  of features on chipped images. Contains output product coordinates and
  corresponding full image coordinates for the four corners of intelligent data.
  
  Fixed length: 224 bytes
  
  Reference: STDI-0002 Volume 1, Appendix B - ICHIPB

seq:
  - id: XFRM_FLAG
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Non-linear Transformation Flag
      00 = non-dewarped, data provided
      01 = no data provided (remaining fields zero-filled)

  - id: SCALE_FACTOR
    type: str
    size: 10
    encoding: BCS-N
    doc: |
      Scale Factor Relative to R0 (original full image resolution)
      Format: xxxx.xxxxx
      Typically reciprocal of display magnification.
      Values: 0001.00000=R0, 0002.00000=R1, 0004.00000=R2, etc.

  - id: ANAMRPH_CORR
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Anamorphic Correction Indicator
      00 = no anamorphic correction
      01 = anamorphic correction applied

  - id: SCANBLK_NUM
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Scan Block Number (scan block index)
      Range: 00-99
      00 if not applicable

  - id: OP_ROW_11
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Output product row number component of grid point index (1,1)
      for intelligent data. Format: xxxxxxxx.yyy
      Typically 00000000.500

  - id: OP_COL_11
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Output product column number component of grid point index (1,1)
      for intelligent data. Format: xxxxxxxx.yyy
      Typically 00000000.500

  - id: OP_ROW_12
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Output product row number component of grid point index (1,2)
      for intelligent data. Format: xxxxxxxx.yyy

  - id: OP_COL_12
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Output product column number component of grid point index (1,2)
      for intelligent data. Format: xxxxxxxx.yyy

  - id: OP_ROW_21
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Output product row number component of grid point index (2,1)
      for intelligent data. Format: xxxxxxxx.yyy

  - id: OP_COL_21
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Output product column number component of grid point index (2,1)
      for intelligent data. Format: xxxxxxxx.yyy

  - id: OP_ROW_22
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Output product row number component of grid point index (2,2)
      for intelligent data. Format: xxxxxxxx.yyy

  - id: OP_COL_22
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Output product column number component of grid point index (2,2)
      for intelligent data. Format: xxxxxxxx.yyy

  - id: FI_ROW_11
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Grid point (1,1), row number in full image coordinate system.
      Format: xxxxxxxx.yyy

  - id: FI_COL_11
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Grid point (1,1), column number in full image coordinate system.
      Format: xxxxxxxx.yyy

  - id: FI_ROW_12
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Grid point (1,2), row number in full image coordinate system.
      Format: xxxxxxxx.yyy

  - id: FI_COL_12
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Grid point (1,2), column number in full image coordinate system.
      Format: xxxxxxxx.yyy

  - id: FI_ROW_21
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Grid point (2,1), row number in full image coordinate system.
      Format: xxxxxxxx.yyy

  - id: FI_COL_21
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Grid point (2,1), column number in full image coordinate system.
      Format: xxxxxxxx.yyy

  - id: FI_ROW_22
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Grid point (2,2), row number in full image coordinate system.
      Format: xxxxxxxx.yyy

  - id: FI_COL_22
    type: str
    size: 12
    encoding: BCS-N
    doc: |
      Grid point (2,2), column number in full image coordinate system.
      Format: xxxxxxxx.yyy

  - id: FI_ROW
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Full Image Number of Rows
      Range: 00000000 (unknown) or 00000002-99999999

  - id: FI_COL
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Full Image Number of Columns
      Range: 00000000 (unknown) or 00000002-99999999
