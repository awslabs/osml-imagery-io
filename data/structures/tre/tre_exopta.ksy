meta:
  id: tre_exopta
  title: Exploitation Usability Optical Information TRE
  endian: be

doc: |
  EXOPTA TRE - Exploitation Usability Optical Information Extension
  
  Optional extension providing metadata to determine if image is suitable
  for exploitation. Contains fields for NGA standard directory entry.
  A single EXOPTA is placed in the image subheader.
  
  Reference: STDI-0002 Volume 1, Appendix E - ASDE

seq:
  - id: angle_to_north
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Angle to True North (ANGLE_TO_NORTH)
      Angle measured clockwise from first-row vector to True North.
      3 BCS-N, 000-359 degrees.

  - id: mean_gsd
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Mean Ground Sample Distance (MEAN_GSD)
      Geometric mean of cross/along scan center-to-center distance.
      5 BCS-N, 000.0-999.9 inches. Accuracy ±10%.

  - id: reserved_001
    type: str
    size: 1
    encoding: ASCII
    doc: Reserved (1 character)

  - id: dynamic_range
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Dynamic Range (DYNAMIC_RANGE)
      Dynamic range of image pixels.
      5 BCS-N, 00000-65535.

  - id: reserved_002
    type: str
    size: 7
    encoding: ASCII
    doc: Reserved (7 spaces)

  - id: obl_ang
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Obliquity Angle (OBL_ANG)
      Angle between local NED horizontal and optical axis.
      5 BCS-N, 00.00-90.00 degrees.

  - id: roll_ang
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Roll Angle (ROLL_ANG)
      Roll angle of platform body.
      6 BCS-N, ±90.00 degrees.

  - id: prime_id
    type: str
    size: 12
    encoding: ASCII
    doc: |
      Primary Target ID (PRIME_ID)
      12 BCS-A.

  - id: prime_be
    type: str
    size: 15
    encoding: ASCII
    doc: |
      Primary Target BE (PRIME_BE)
      Basic Encyclopedia or non-BE ID of primary target.
      15 BCS-A.

  - id: reserved_003
    type: str
    size: 5
    encoding: ASCII
    doc: Reserved (5 spaces)

  - id: n_sec
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Number of Secondary Targets (N_SEC)
      Number of SECTG extensions present.
      3 BCS-N, 000-250.

  - id: reserved_004
    type: str
    size: 2
    encoding: ASCII
    doc: Reserved (2 spaces)

  - id: reserved_005
    type: str
    size: 7
    encoding: ASCII
    doc: Reserved (value 0000001)

  - id: n_seg
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Number of Segments (N_SEG)
      Separate imagery pieces within an imaging operation.
      3 BCS-N, 001-999.

  - id: max_lp_seg
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Maximum Lines Per Segment (MAX_LP_SEG)
      Maximum number of lines per segment including overlap.
      6 BCS-N, 000001-199999.

  - id: reserved_006
    type: str
    size: 12
    encoding: ASCII
    doc: Reserved (12 spaces)

  - id: sun_el
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Sun Elevation (SUN_EL)
      Angle from target plane at first image line.
      5 BCS-N, ±90.0 degrees, or 999.9 if unavailable.

  - id: sun_az
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Sun Azimuth (SUN_AZ)
      Angle from True North clockwise at first image line.
      5 BCS-N, 000.0-359.9 degrees, or 999.9 if unavailable.
