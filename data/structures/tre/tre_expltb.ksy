meta:
  id: tre_expltb
  title: Exploitation Related Information TRE
  endian: be

doc: |
  EXPLTB TRE - Exploitation Related Information Extension - Version B
  
  Optional extension providing metadata to determine if image is suitable
  for exploitation. Contains fields for NGA standard directory entry.
  A single EXPLTB is placed in the image subheader.
  
  Reference: STDI-0002 Volume 1, Appendix E - ASDE

seq:
  - id: angle_to_north
    type: str
    size: 7
    encoding: ASCII
    doc: |
      Angle to True North (ANGLE_TO_NORTH)
      Angle measured clockwise from first-row vector to True North.
      7 BCS-N, 000.000-359.999 degrees.

  - id: angle_to_north_accy
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Angle to North Accuracy (ANGLE_TO_NORTH_ACCY)
      90% probable error value.
      6 BCS-N, 00.001-44.999 degrees, or 000000/00.000 for unknown.

  - id: squint_angle
    type: str
    size: 7
    encoding: ASCII
    doc: |
      Squint Angle (SQUINT_ANGLE)
      Angle from crosstrack to great circle joining ARP to ORP.
      7 BCS-N, -60.000 to +85.000 degrees.

  - id: squint_angle_accy
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Squint Angle Accuracy (SQUINT_ANGLE_ACCY)
      90% probable error value.
      6 BCS-N, 00.001-44.999 degrees, or 000000/00.000 for unknown.

  - id: mode
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Mode (MODE)
      Collection mode and processing mode.
      3 BCS-A, sensor-specific coded value.

  - id: reserved_001
    type: str
    size: 16
    encoding: ASCII
    doc: Reserved (16 spaces)

  - id: graze_ang
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Grazing Angle (GRAZE_ANG)
      Angle between focus plane and line of sight to radar.
      5 BCS-N, 00.00-90.00 degrees.

  - id: graze_ang_accy
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Grazing Angle Accuracy (GRAZE_ANG_ACCY)
      90% probable error value.
      5 BCS-N, 00.01-90.00 degrees, or 00000/00.00 for unknown.

  - id: slope_ang
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Slope Angle (SLOPE_ANG)
      Angle between SAR plane and focus plane.
      5 BCS-N, 00.00-90.00 degrees.

  - id: polar
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Polarization (POLAR)
      Transmit and receive polarization.
      2 BCS-A, HH/HV/VH/VV.

  - id: nsamp
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Pixels per Line (NSAMP)
      Includes fill pixels.
      5 BCS-N, 00001-99999.

  - id: reserved_002
    type: str
    size: 1
    encoding: ASCII
    doc: Reserved (value 0)

  - id: seq_num
    type: str
    size: 1
    encoding: ASCII
    doc: |
      Sequence Number (SEQ_NUM)
      Sequence within coupled imagery set.
      1 BCS-N, 1-6.

  - id: prime_id
    type: str
    size: 12
    encoding: ASCII
    doc: |
      Primary Target ID (PRIME_ID)
      Target designator of primary target.
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
    size: 1
    encoding: ASCII
    doc: Reserved (value 0)

  - id: n_sec
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Number of Secondary Targets (N_SEC)
      Number of SECTGA extensions.
      2 BCS-N, 00-99.

  - id: ipr
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Commanded Impulse Response (IPR)
      2 BCS-N, 00-99 feet. 00 for unknown.
