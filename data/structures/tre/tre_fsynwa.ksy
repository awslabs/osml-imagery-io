meta:
  id: tre_fsynwa
  title: Frame Synchronous Wrapper TRE
  endian: be

doc: |
  FSYNWA TRE - Frame-Synchronous Metadata Wrapper Tagged Record Extension

  Wraps one or more other NITF TREs and associates them with a specific
  frame or consecutive range of frames in a temporal block (NITF image
  segment). This allows metadata TREs that predate MIE4NITF to be
  associated with specific frames without modification.

  Multiple FSYNWA TREs may appear in an image segment subheader, each
  wrapping different sets of metadata TREs for different frame ranges.

  Precedence: When overlapping frame ranges contain the same TRE type,
  the FSYNWA TRE with the higher byte offset (later in the file) takes
  precedence. FSYNWA-wrapped TREs also take precedence over the same
  TRE type found in a FASYWA wrapper.

  All bytes within CEDATA must be accounted for by the contained TREs.
  No unused bytes are allowed unless contained within a FREESA TRE.

  This TRE is part of the Motion Imagery Extensions for NITF 2.1 (MIE4NITF)
  specification defined in NGA.STND.0044.

  Reference: STDI-0002 Volume 1, Appendix AF, Section AF 5.10
  Reference: NGA.STND.0044_1.3.3 - Motion Imagery Extension for NITF 2.1

seq:
  - id: START_FRAME_NUMBER
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Start Frame Number
      9 BCS-N positive integer. First frame number to which the
      wrapped TREs apply. Must be >= START_FRAME_NUMBER of any
      FSYNWA TRE found earlier in the image subheader.

  - id: END_FRAME_NUMBER
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      End Frame Number
      9 BCS-N positive integer. Last frame number to which the
      wrapped TREs apply.

  - id: CEDATA
    size-eos: true
    doc: |
      Contained Extension Data
      Variable length. Contains one or more complete NITF TREs
      (each with their own CETAG/CEL/CEDATA structure).
      All bytes must be accounted for by the contained TREs.
