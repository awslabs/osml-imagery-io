meta:
  id: tre_fasywa
  title: Frame Asynchronous Wrapper TRE
  endian: be

doc: |
  FASYWA TRE - Frame-Asynchronous Metadata Wrapper Tagged Record Extension

  Wraps one or more other NITF TREs and associates them with a specific
  point or period of time in the collection. Used for metadata that is
  collected at a point in time but not necessarily tied to a specific frame
  (e.g., platform navigation information).

  May be placed in a NITF file header or image segment subheader depending
  on the scope of the metadata being wrapped.

  Precedence: When overlapping time ranges contain the same TRE type,
  the FASYWA TRE with the higher byte offset (later in the file) takes
  precedence.

  All bytes within CEDATA must be accounted for by the contained TREs.
  No unused bytes are allowed unless contained within a FREESA TRE.

  This TRE is part of the Motion Imagery Extensions for NITF 2.1 (MIE4NITF)
  specification defined in NGA.STND.0044.

  Reference: STDI-0002 Volume 1, Appendix AF, Section AF 5.9
  Reference: NGA.STND.0044_1.3.3 - Motion Imagery Extension for NITF 2.1

seq:
  - id: START_TIMESTAMP
    type: str
    size: 24
    encoding: BCS-A
    doc: |
      Start Timestamp
      24 BCS-A. UTC timestamp (YYYYMMDDHHmmSS.fffffffff---).
      Start of the time period to which the wrapped TREs apply.
      Must be >= START_TIMESTAMP of any FASYWA TRE found earlier
      in the image subheader or file header.

  - id: END_TIMESTAMP
    type: str
    size: 24
    encoding: BCS-A
    doc: |
      End Timestamp
      24 BCS-A. UTC timestamp (YYYYMMDDHHmmSS.fffffffff---).
      End of the time period to which the wrapped TREs apply.

  - id: CEDATA
    size-eos: true
    doc: |
      Contained Extension Data
      Variable length. Contains one or more complete NITF TREs
      (each with their own CETAG/CEL/CEDATA structure).
      All bytes must be accounted for by the contained TREs.
