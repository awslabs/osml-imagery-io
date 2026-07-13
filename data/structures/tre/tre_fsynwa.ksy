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
    type: encapsulated_tre
    repeat: eos
    doc: |
      Contained Extension Data
      One or more complete NITF TREs, each modeled as a
      TRETAGn/TRELn/TREDATAn group. The number of encapsulated TREs is
      not signaled by a count field; the group repeats until the end of
      the TRE data (repeat: eos). All bytes must be accounted for by the
      contained TREs.

types:
  encapsulated_tre:
    doc: |
      One encapsulated NITF TRE, as it appears inside FSYNWA's CEDATA.
      Per Table AF-10 (p. AF-38/AF-39) the group is TRETAGn, TRELn, and
      TREDATAn, repeated once per encapsulated TRE.
    seq:
      - id: TRETAG
        type: str
        size: 6
        encoding: BCS-A
        doc: |
          The TRETAG of the nth encapsulated TRE (6 BCS-A).

      - id: TREL
        type: str
        size: 5
        encoding: BCS-N
        doc: |
          The length in bytes of the TREDATA field of the nth encapsulated
          TRE (5 BCS-N, 00001 - 99956). If this value is zero, the TREDATA
          field for the nth TRE is not present (size 0).

      - id: TREDATA
        type: bytes
        size: TREL.to_i
        doc: |
          The data of the nth encapsulated TRE. Its length is given by
          TRELn. Raw bytes (an entire encapsulated TRE payload), so it is
          modeled as `bytes` for faithful round-trip regardless of content.
