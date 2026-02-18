meta:
  id: tre_blocka
  title: Image Block Information TRE
  endian: be

doc: |
  BLOCKA TRE - Image Block Information Extension
  
  Optional but often needed for exploitation of imagery.
  Placed in the image subheader with corresponding AIMID and ACFT extensions.
  Provides higher precision corner coordinates than IGEOLO.
  
  Reference: STDI-0002 Volume 1, Appendix E - ASDE

seq:
  - id: block_instance
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Block Instance (BLOCK_INSTANCE)
      Block number of this image block.
      2 BCS-N, 01-99.

  - id: n_gray
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Number of Gray Fill Pixels (N_GRAY)
      SAR: number of gray fill pixels. EO-IR: 00000.
      5 BCS-N, 00000-99999.

  - id: l_lines
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Row Count (L_LINES)
      5 BCS-N, 00001-99999.

  - id: layover_angle
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Layover Angle (LAYOVER_ANGLE)
      SAR: angle between first row and layover direction.
      3 BCS-N, 000-359 degrees, or spaces for EO-IR.

  - id: shadow_angle
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Shadow Angle (SHADOW_ANGLE)
      SAR: angle between first row and radar shadow.
      3 BCS-N, 000-359 degrees, or spaces for EO-IR.

  - id: reserved_001
    type: str
    size: 16
    encoding: ASCII
    doc: Reserved (16 spaces)

  - id: frlc_loc
    type: str
    size: 21
    encoding: ASCII
    doc: |
      First Row Last Column Location (FRLC_LOC)
      High precision corner coordinate.
      21 BCS-A, Xddmmss.ssYdddmmss.ss or ±dd.dddddd±ddd.dddddd.

  - id: lrlc_loc
    type: str
    size: 21
    encoding: ASCII
    doc: |
      Last Row Last Column Location (LRLC_LOC)
      High precision corner coordinate.
      21 BCS-A.

  - id: lrfc_loc
    type: str
    size: 21
    encoding: ASCII
    doc: |
      Last Row First Column Location (LRFC_LOC)
      High precision corner coordinate.
      21 BCS-A.

  - id: frfc_loc
    type: str
    size: 21
    encoding: ASCII
    doc: |
      First Row First Column Location (FRFC_LOC)
      High precision corner coordinate.
      21 BCS-A.

  - id: reserved_002
    type: str
    size: 5
    encoding: ASCII
    doc: Reserved (value 010.0)
