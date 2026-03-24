meta:
  id: tre_mtimfa
  title: Motion Imagery File TRE
  endian: be

doc: |
  MTIMFA TRE - Motion Imagery Temporal Block Mapping Tagged Record Extension

  Specifies how the MI data for all cameras in a phenomenological layer for
  a given camera set and time interval are subdivided into temporal blocks
  and associates those temporal blocks with NITF image segment indices.

  One MTIMFA TRE is placed in the NITF file header for each phenomenological
  layer in a file.

  This TRE is part of the Motion Imagery Extensions for NITF 2.1 (MIE4NITF)
  specification defined in NGA.STND.0044.

  Reference: STDI-0002 Volume 1, Appendix AF, Section AF 5.5
  Reference: NGA.STND.0044_1.3.3 - Motion Imagery Extension for NITF 2.1

seq:
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
      3 BCS-N positive integer. Index of the camera set.

  - id: TIME_INTERVAL_INDEX
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Time Interval Index
      6 BCS-N positive integer. Index of the time interval.

  - id: NUM_CAMERAS_DEFINED
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Number of Cameras Defined
      3 BCS-N positive integer. Number of cameras in this layer/set.

  - id: CAMERAS
    type: camera_temporal_record
    repeat: expr
    repeat-expr: NUM_CAMERAS_DEFINED.to_i
    doc: Per-camera temporal block definitions.

types:
  camera_temporal_record:
    seq:
      - id: CAMERA_ID
        type: str
        size: 36
        encoding: BCS-A
        doc: |
          Camera UUID
          36 BCS-A UUID (X.667 format).

      - id: NUM_TEMP_BLOCKS
        type: str
        size: 3
        encoding: BCS-N
        doc: |
          Number of Temporal Blocks
          3 BCS-N positive integer. Number of temporal blocks for this camera.

      - id: TEMPORAL_BLOCKS
        type: temporal_block_record
        repeat: expr
        repeat-expr: NUM_TEMP_BLOCKS.to_i
        doc: Temporal block definitions for this camera.

  temporal_block_record:
    seq:
      - id: START_TIMESTAMP
        type: str
        size: 24
        encoding: BCS-A
        doc: |
          Start Timestamp
          24 BCS-A. UTC timestamp (YYYYMMDDHHmmSS.fffffffff---).
          Start time of this temporal block.

      - id: END_TIMESTAMP
        type: str
        size: 24
        encoding: BCS-A
        doc: |
          End Timestamp
          24 BCS-A. UTC timestamp (YYYYMMDDHHmmSS.fffffffff---).
          End time of this temporal block.

      - id: IMAGE_SEG_INDEX
        type: str
        size: 3
        encoding: BCS-A
        doc: |
          Image Segment Index
          3 BCS-A. BCS-N positive integer or BCS-A spaces.
          Index of the NITF image segment containing this temporal block.
          All spaces if the temporal block was not collected (dropped).
