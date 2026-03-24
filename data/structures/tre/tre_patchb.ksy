meta:
  id: tre_patchb
  title: SAR Patch Data TRE
  endian: be

doc: |
  PATCHB TRE - SAR Patch Data
  
  Provides SAR (Synthetic Aperture Radar) patch-level data
  including patch geometry, timing, platform velocity,
  and navigation information.
  
  Fixed length: 121 bytes.
  
  Reference: STDI-0002 Volume 1, Appendix E, Section E.3.11.2, Table E-21

seq:
  - id: PAT_NO
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Patch Number
      4 BCS-N integer, range 1-999.

  - id: LAST_PAT_FLAG
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Last Patch Flag
      1 BCS-N integer, 0=not last, 1=last.

  - id: LNSTRT
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Line Start
      7 BCS-N integer, range 1-9999999.

  - id: LNSTOP
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Line Stop
      7 BCS-N integer, range 20-9999999.

  - id: AZL
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Azimuth Lines
      5 BCS-N integer, lines, range 20-99999.

  - id: NVL
    type: str
    size: 5
    encoding: BCS-A
    doc: |
      Number of Valid Lines
      5 BCS-A (may contain spaces).

  - id: FVL
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      First Valid Line
      3 BCS-A, range 1-681 (may contain spaces).

  - id: NPIXEL
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Number of Pixels
      5 BCS-N integer, pixels, range 1-99999.

  - id: FVPIX
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      First Valid Pixel
      5 BCS-N integer, pixels, range 1-99999.

  - id: FRAME
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Frame Number
      3 BCS-A, range 1-512 (may contain spaces).

  - id: UTC
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      UTC Time
      8 BCS-N real, seconds, range 0.0-86399.99.

  - id: SHEAD
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Sensor Heading
      7 BCS-N real, degrees, range 0.0-359.999.

  - id: GRAVITY
    type: str
    size: 7
    encoding: BCS-A
    doc: |
      Gravity
      7 BCS-A, feet/sec^2 (may contain spaces).

  - id: INS_V_NC
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      INS Velocity North Component
      5 BCS-N integer, feet/sec, range -9999 to 9999.

  - id: INS_V_EC
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      INS Velocity East Component
      5 BCS-N integer, feet/sec, range -9999 to 9999.

  - id: INS_V_DC
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      INS Velocity Down Component
      5 BCS-N integer, feet/sec, range -9999 to 9999.

  - id: OFFLAT
    type: str
    size: 8
    encoding: BCS-A
    doc: |
      Offset Latitude
      8 BCS-A, seconds (may contain spaces).

  - id: OFFLONG
    type: str
    size: 8
    encoding: BCS-A
    doc: |
      Offset Longitude
      8 BCS-A, seconds (may contain spaces).

  - id: TRACK
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Track Angle
      3 BCS-N integer, degrees, range 0-359.

  - id: GSWEEP
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Ground Sweep
      6 BCS-N real, degrees, range 0.0-120.0.

  - id: SHEAR
    type: str
    size: 8
    encoding: BCS-A
    doc: |
      Shear
      8 BCS-A (may contain spaces).

  - id: BATCH_NO
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      Batch Number
      6 BCS-A (may contain spaces).
