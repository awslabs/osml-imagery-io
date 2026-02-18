meta:
  id: tre_use00a
  title: Exploitation Usability TRE
  endian: be

doc: |
  USE00A TRE - Exploitation Usability Extension
  
  Allows a user program to determine if the image is usable for the
  exploitation problem currently being performed. Also contains
  catalogue metadata.
  
  Reference: STDI-0002 Volume 1, Appendix D - CSDE

seq:
  - id: angle_to_north
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Angle to North (ANGLE_TO_NORTH)
      Angle measured clockwise from first-row vector to True North.
      3 BCS-N, 000-359 degrees.

  - id: mean_gsd
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Mean Ground Sample Distance (MEAN_GSD)
      Geometric mean of cross/along scan center-to-center distance.
      5 BCS-N, 000.0-999.9 inches. Accuracy +10%.

  - id: reserved1
    type: str
    size: 1
    encoding: ASCII
    doc: Reserved (1 space)

  - id: dynamic_range
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Dynamic Range (DYNAMIC_RANGE)
      Dynamic range of pixels in image.
      5 BCS-N, 00000-99999.

  - id: reserved2
    type: str
    size: 3
    encoding: ASCII
    doc: Reserved (3 spaces)

  - id: reserved3
    type: str
    size: 1
    encoding: ASCII
    doc: Reserved (1 space)

  - id: reserved4
    type: str
    size: 3
    encoding: ASCII
    doc: Reserved (3 spaces)

  - id: obl_ang
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Obliquity Angle (OBL_ANG)
      5 BCS-N, 00.00-90.00 degrees.

  - id: roll_ang
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Roll Angle (ROLL_ANG)
      6 BCS-N, +90.00 degrees (signed).

  - id: reserved5
    type: str
    size: 12
    encoding: ASCII
    doc: Reserved (12 spaces)

  - id: reserved6
    type: str
    size: 15
    encoding: ASCII
    doc: Reserved (15 spaces)

  - id: reserved7
    type: str
    size: 4
    encoding: ASCII
    doc: Reserved (4 spaces)

  - id: reserved8
    type: str
    size: 1
    encoding: ASCII
    doc: Reserved (1 space)

  - id: reserved9
    type: str
    size: 3
    encoding: ASCII
    doc: Reserved (3 spaces)

  - id: reserved10
    type: str
    size: 1
    encoding: ASCII
    doc: Reserved (1 space)

  - id: reserved11
    type: str
    size: 1
    encoding: ASCII
    doc: Reserved (1 space)

  - id: n_ref
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Number of Reference Lines (N_REF)
      Number of reference lines in image.
      2 BCS-N, 00-99.

  - id: rev_num
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Revolution Number (REV_NUM)
      Revolution number at northernmost point of orbit.
      5 BCS-N, 00001-99999.

  - id: n_seg
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Number of Segments (N_SEG)
      3 BCS-N, 001-999.

  - id: max_lp_seg
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Maximum Lines Per Segment (MAX_LP_SEG)
      Maximum number of lines per segment including overlap.
      6 BCS-N, 000001-999999.

  - id: reserved12
    type: str
    size: 6
    encoding: ASCII
    doc: Reserved (6 spaces)

  - id: reserved13
    type: str
    size: 6
    encoding: ASCII
    doc: Reserved (6 spaces)

  - id: sun_el
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Sun Elevation (SUN_EL)
      Sun elevation from target plane at first image line.
      5 BCS-N, -90.0 to +90.0 degrees, or 999.9 if unavailable.

  - id: sun_az
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Sun Azimuth (SUN_AZ)
      Sun azimuth from True North clockwise at first image line.
      5 BCS-N, 000.0-359.0 degrees, or 999.9 if unavailable.
