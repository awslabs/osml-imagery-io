meta:
  id: tre_matesa
  title: Mates TRE
  endian: be

doc: |
  MATESA TRE - Mates Tagged Record Extension
  
  Provides a means for data providers to specify files and collections of
  other images that are all related to each other in some fashion, also
  known as "mates." The MATESA TRE may reside in the NITF file header
  or the image segment subheader.
  
  Supports use cases including:
  - Coordinated tasking (triangulation, stereo pairs, multi-look, multi-view)
  - Related products (mosaics, sharpening, data fusion)
  
  Reference: STDI-0002 Volume 1, Appendix AK - MATESA

seq:
  - id: CUR_SOURCE
    type: str
    size: 42
    encoding: ECS-A
    doc: |
      Current File/Segment Source (CUR_SOURCE)
      Source of the current file or file segment. For images, the source
      is the sensor name and model. For IDPs, where the sensor name is
      no longer relevant (such as textual products), the name of the mate
      producer shall be provided.
      42 ECS-A characters.

  - id: CUR_MATE_TYPE
    type: str
    size: 16
    encoding: ECS-A
    doc: |
      Current File/Segment Mate Type (CUR_MATE_TYPE)
      The type of mate identifier used to specify the current file or segment.
      Values include: "FILEID", "NITF_IID2", "NITF_FTITLE", "SICD_CORENAME",
      "SIDD_IIDL", "STDI_UID", "NITF_IDATIM", "NITF_ISORCE", "NITF_FSCLAS",
      "NITF_FSCLTX", "NITF_FSCATP", "NITF_FSCRSN", "NITF_FSDCTP", "NITF_FSDCDT",
      "NITF_FSDCXM", "NITF_FSDG", "NITF_FSDGDT", "NITF_FSCLTX", "NITF_FSCATP".
      16 ECS-A characters.

  - id: CUR_FILE_ID_LEN
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Length of the CUR_FILE_ID field (CUR_FILE_ID_LEN)
      This field contains the length in bytes of the CUR_FILE_ID field.
      4 BCS-N characters, range 0001-9999.

  - id: CUR_FILE_ID
    type: str
    size: CUR_FILE_ID_LEN.to_i
    encoding: ECS-A
    doc: |
      ID of the Current File/Segment (CUR_FILE_ID)
      This field records the ID of the current file or segment, i.e.,
      the file/segment to which all files identified in the MATESA TRE
      are mates.
      Variable length (1-9999 bytes) ECS-A characters.

  - id: NUM_GROUPS
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Number of Mate Relationship Groups (NUM_GROUPS)
      Number of mate relationship groups.
      4 BCS-N characters, range 0001-9999.

  - id: GROUPS
    type: mate_group
    repeat: expr
    repeat-expr: NUM_GROUPS.to_i
    doc: |
      Mate relationship groups.
      Repeated NUM_GROUPS times.

types:
  mate_group:
    seq:
      - id: RELATIONSHIP
        type: str
        size: 24
        encoding: ECS-A
        doc: |
          Mate Relationship (RELATIONSHIP)
          This field reports the relationship of the nth group of related
          files to the current file.
          Values for coordinated collections: "TRIANGULATION", "STEREO",
          "MULTI-LOOK", "MULTI-VIEW", "MULTI-PHENOMENOLOGY", "CONCURRENT".
          Values for data products: "MOSAIC", "SHARPENING", "DATA_FUSION",
          "CHANGE_DETECTION", "DERIVED_FROM", "DERIVED_TO", etc.
          24 ECS-A characters.

      - id: NUM_MATES
        type: str
        size: 4
        encoding: BCS-N
        doc: |
          Number of Mates in the nth Group (NUM_MATES)
          This field reports the number of mates in the nth group of
          related files.
          4 BCS-N characters, range 0001-9999.

      - id: MATES
        type: mate_entry
        repeat: expr
        repeat-expr: NUM_MATES.to_i
        doc: |
          Mate entries for this group.
          Repeated NUM_MATES times.

  mate_entry:
    seq:
      - id: SOURCE
        type: str
        size: 42
        encoding: ECS-A
        doc: |
          Mate Source (SOURCE)
          Source of the mth mate in the nth group of related files.
          For images, the source is the sensor name and model.
          For IDPs, where the sensor name is no longer relevant
          (such as textual products), the name of the mate producer
          shall be provided.
          42 ECS-A characters.

      - id: MATE_TYPE
        type: str
        size: 16
        encoding: ECS-A
        doc: |
          Mate Identifier Type (MATE_TYPE)
          The type of mate ID used to specify the mth mate of the
          nth group of related files.
          16 ECS-A characters.

      - id: MATE_ID_LEN
        type: str
        size: 4
        encoding: BCS-N
        doc: |
          Length of the MATE_ID field (MATE_ID_LEN)
          This field contains the length in bytes of the MATE_ID field
          for the mth mate in the nth group of related files.
          4 BCS-N characters, range 0001-9999.

      - id: MATE_ID
        type: str
        size: MATE_ID_LEN.to_i
        encoding: ECS-A
        doc: |
          Mate File Identifier (MATE_ID)
          This field contains the ID of the mth mate of the nth group
          of related files.
          Variable length (1-9999 bytes) ECS-A characters.

