meta:
  id: tre_mtimsa
  title: Motion Imagery Segment TRE
  endian: be

doc: |
  MTIMSA TRE - Motion Imagery Timing Tagged Record Extension

  Specifies the nominal frame rate, frame numbers, and timestamps for the
  MI data within the image segment in which the TRE is found. Ties this
  information back to the phenomenological layer, camera set, camera, time
  interval, and temporal block associated with the image segment.

  The frame timestamps in the MTIMSA TRE take precedence over any other
  NITF TRE when associating time to an MI frame.

  This TRE uses the UINTn binary data type for DT_MULTIPLIER, DT_SIZE,
  NUMBER_FRAMES, NUMBER_DT, and DTn fields. These are big-endian unsigned
  integers.

  Frame timestamp calculation:
    T(1) = BASE_TIMESTAMP + DT(1) * DT_MULTIPLIER
    T(n) = T(n-1) + DT(n) * DT_MULTIPLIER  (for n > 1)

  When NUMBER_DT = 1, a single DT value applies to all frames (constant
  frame rate). When NUMBER_DT > 1, each DT value specifies the delta to
  the next frame (variable frame rate).

  This TRE is part of the Motion Imagery Extensions for NITF 2.1 (MIE4NITF)
  specification defined in NGA.STND.0044.

  Reference: STDI-0002 Volume 1, Appendix AF, Section AF 5.7
  Reference: NGA.STND.0044_1.3.3 - Motion Imagery Extension for NITF 2.1

seq:
  - id: IMAGE_SEG_INDEX
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Image Segment Index
      3 BCS-N positive integer. Index of the NITF image segment.

  - id: GEOCOORDS_STATIC
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Geocoordinates Static Flag
      2 BCS-N positive integer. Indicates whether IGEOLO is static
      across all frames (00 = static or not applicable).

  - id: LAYER_ID
    type: str
    size: 36
    encoding: BCS-A
    doc: |
      Phenomenological Layer UUID
      36 BCS-A. UUID identifying the phenomenological layer.

  - id: CAMERA_SET_INDEX
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Camera Set Index
      3 BCS-N positive integer.

  - id: CAMERA_ID
    type: str
    size: 36
    encoding: BCS-A
    doc: |
      Camera UUID
      36 BCS-A UUID (X.667 format).

  - id: TIME_INTERVAL_INDEX
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Time Interval Index
      6 BCS-N positive integer.

  - id: TEMP_BLOCK_INDEX
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Temporal Block Index
      3 BCS-N positive integer.

  - id: NOMINAL_FRAME_RATE
    type: str
    size: 13
    encoding: BCS-A
    doc: |
      Nominal Frame Rate
      13 BCS-A, UE/13 scientific notation. Frames per second.

  - id: REFERENCE_FRAME_NUM
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Reference Frame Number
      9 BCS-N positive integer. Frame number of the first frame
      in this temporal block within the overall collection.

  - id: BASE_TIMESTAMP
    type: str
    size: 24
    encoding: BCS-A
    doc: |
      Base Timestamp
      24 BCS-A. UTC timestamp (YYYYMMDDHHmmSS.fffffffff---).
      Base time for frame timestamp calculation.

  - id: DT_MULTIPLIER
    type: u8
    doc: |
      Delta Time Multiplier
      UINT64 (8 bytes). Multiplier applied to DTn values to compute
      frame time deltas in nanoseconds.

  - id: DT_SIZE
    type: u1
    doc: |
      Delta Time Size
      UINT8 (1 byte). Size in bytes of each DTn value.
      Determines the UINTn type: n = 8 * DT_SIZE.

  - id: NUMBER_FRAMES
    type: u4
    doc: |
      Number of Frames
      UINT32 (4 bytes). Total number of frames in this temporal block.

  - id: NUMBER_DT
    type: u4
    doc: |
      Number of DT Values
      UINT32 (4 bytes). Number of DTn values that follow.
      If 1, a single DT applies to all frames (constant rate).
      Range: 0 to NUMBER_FRAMES - 1 (0 for single-frame segments).

  - id: DT
    size: DT_SIZE
    repeat: expr
    repeat-expr: NUMBER_DT
    doc: |
      Delta Time Values
      UINTn (n = 8 * DT_SIZE) repeated NUMBER_DT times.
      Each value is a time delta. Multiply by DT_MULTIPLIER to get
      nanoseconds between frames.
