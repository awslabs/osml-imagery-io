meta:
  id: tre_mimcsa
  title: Motion Imagery Collection Summary TRE
  endian: be

doc: |
  MIMCSA TRE - Motion Imagery Collection Summary

  Contains high-level metadata regarding the frame rate range of the motion
  imagery, encoding methods used, and if any temporal subsampling was
  performed. One MIMCSA TRE is placed in the NITF file header for each
  phenomenological layer present in the collection.

  This TRE is part of the Motion Imagery Extensions for NITF 2.1 (MIE4NITF)
  specification defined in NGA.STND.0044.

  Reference: STDI-0002 Volume 1, Appendix AF, Section AF 5.1
  Reference: NGA.STND.0044_1.3.3 - Motion Imagery Extension for NITF 2.1

seq:
  - id: LAYER_ID
    type: str
    size: 36
    encoding: BCS-A
    doc: |
      Phenomenological Layer UUID
      36 BCS-A. UUID identifying the phenomenological layer.

  - id: NOMINAL_FRAME_RATE
    type: str
    size: 13
    encoding: BCS-A
    doc: |
      Nominal Frame Rate
      13 BCS-A, UE/13 scientific notation. Frames per second.
      May be NaN if unknown at time of writing.

  - id: MIN_FRAME_RATE
    type: str
    size: 13
    encoding: BCS-A
    doc: |
      Minimum Frame Rate
      13 BCS-A, UE/13 scientific notation. Frames per second.
      May be NaN if unknown at time of writing.

  - id: MAX_FRAME_RATE
    type: str
    size: 13
    encoding: BCS-A
    doc: |
      Maximum Frame Rate
      13 BCS-A, UE/13 scientific notation. Frames per second.
      May be NaN if unknown at time of writing.

  - id: T_RSET
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Temporal Rate Set
      2 BCS-N positive integer. Temporal subsampling indicator.
      00 = no subsampling.

  - id: MI_REQ_DECODER
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Required MI Decoder
      2 BCS-A. Any legal IC field value indicating the decoder
      required to interpret the motion imagery data.

  - id: MI_REQ_PROFILE
    type: str
    size: 36
    encoding: BCS-A
    doc: |
      Required MI Decoder Profile
      36 BCS-A. Profile name for the required decoder.
      Space-padded if not applicable.

  - id: MI_REQ_LEVEL
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      Required MI Decoder Level
      6 BCS-A. Level for the required decoder.
      Space-padded if not applicable.
