meta:
  id: tre_contxa
  title: Context Metadata Wrapper TRE
  endian: be

doc: |
  CONTXA TRE - Context Metadata Wrapper Tagged Record Extension

  Provides a mechanism to associate metadata TREs with producer-defined
  contexts. A context can be as simple as a collection of frames or
  temporal blocks, or it can be a more complex construct. The CONTXA TRE
  may even wrap itself to provide hierarchical context definitions.

  May be placed in a manifest file header, NITF file header, or NITF
  image segment subheader depending on the context type.

  All bytes within CEDATA must be accounted for by the contained TREs.
  No unused bytes are allowed unless contained within a FREESA TRE.

  This TRE is part of the Motion Imagery Extensions for NITF 2.1 (MIE4NITF)
  specification defined in NGA.STND.0044.

  Reference: STDI-0002 Volume 1, Appendix AF, Section AF 5.11, Table AF-11
  Reference: NGA.STND.0044_1.3.3 - Motion Imagery Extension for NITF 2.1

seq:
  - id: CONTEXT_TYPE
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Context Type (CONTEXT_TYPE)
      The type of context specified for the wrapped TREs within this
      instance of CONTXA. Values specified in Table AF-13.
      2 BCS-A characters.

  - id: AGGREGATION_MODE
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Aggregation Mode (AGGREGATION_MODE)
      Flag indicating independent or aggregated context.
      Values specified in Table AF-14.
      1 BCS-A character.

  - id: INDEX_LIST_LENGTH
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Index List Length (INDEX_LIST_LENGTH)
      The length in bytes (characters) of the index list string.
      BCS-A characters are one byte characters.
      4 BCS-N characters.
      Range: 0001 to 9999

  - id: INDEX_LIST
    type: str
    size: INDEX_LIST_LENGTH.to_i
    encoding: BCS-A
    doc: |
      Index List (INDEX_LIST)
      The set of indices of elements to which the encapsulated TREs
      are linked. Defined using ABNF syntax (see Figure AF-9).
      Supports individual indices, closed ranges (e.g., "3-15"),
      open ranges (e.g., "5-"), and comma-separated combinations.
      Trailing spaces are allowed and should be ignored.
      Variable length BCS-A, size = INDEX_LIST_LENGTH.

  - id: CEDATA
    size-eos: true
    doc: |
      Encapsulated TRE Data (CEDATA)
      Contains one or more complete NITF TREs. Each encapsulated TRE
      consists of TRETAGn (6 BCS-A), TRELn (5 BCS-N), and TREDATAn
      (TRELn bytes). The number of TREs is not signaled and is
      determined by parsing the TRE data. The CEDATA is captured as
      an opaque blob by design - the wrapped TREs are self-describing
      NITF TREs that can be parsed independently.
