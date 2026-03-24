meta:
  id: tre_camsda
  title: Camera Set Definition TRE
  endian: be

doc: |
  CAMSDA TRE - Camera Set Definition Tagged Record Extension

  Defines the camera sets, places cameras on the NCCS (NITF Common Coordinate
  System), assigns phenomenological layer IDs and UUIDs to all cameras in
  the collection. One CAMSDA TRE is placed in the NITF file header of every
  file in the collection including the manifest file.

  Multiple instances of the CAMSDA TRE may be needed to provide all the
  necessary metadata due to TRE length constraints.

  This TRE is part of the Motion Imagery Extensions for NITF 2.1 (MIE4NITF)
  specification defined in NGA.STND.0044.

  Reference: STDI-0002 Volume 1, Appendix AF, Section AF 5.2
  Reference: NGA.STND.0044_1.3.3 - Motion Imagery Extension for NITF 2.1

seq:
  - id: NUM_CAMERA_SETS
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Total Number of Camera Sets
      3 BCS-N positive integer. Total camera sets in the collection.

  - id: NUM_CAMERA_SETS_IN_TRE
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Number of Camera Sets in This TRE
      3 BCS-N positive integer. Camera sets defined in this TRE instance.

  - id: FIRST_CAMERA_SET_IN_TRE
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      First Camera Set Index in This TRE
      3 BCS-N positive integer. 1-based index of the first camera set
      defined in this TRE instance.

  - id: CAMERA_SETS
    type: camera_set
    repeat: expr
    repeat-expr: NUM_CAMERA_SETS_IN_TRE.to_i
    doc: Camera set definitions.

types:
  camera_set:
    seq:
      - id: NUM_CAMERAS_IN_SET
        type: str
        size: 3
        encoding: BCS-N
        doc: |
          Number of Cameras in This Set
          3 BCS-N positive integer.

      - id: CAMERAS
        type: camera_record
        repeat: expr
        repeat-expr: NUM_CAMERAS_IN_SET.to_i
        doc: Camera definitions within this set.

  camera_record:
    seq:
      - id: CAMERA_ID
        type: str
        size: 36
        encoding: BCS-A
        doc: |
          Camera UUID
          36 BCS-A. UUID identifying this camera (X.667 format).

      - id: CAMERA_DESC
        type: str
        size: 80
        encoding: BCS-A
        doc: |
          Camera Description
          80 BCS-A. Free-text description of the camera.

      - id: LAYER_ID
        type: str
        size: 36
        encoding: BCS-A
        doc: |
          Phenomenological Layer UUID
          36 BCS-A. UUID of the phenomenological layer this camera belongs to.

      - id: IDLVL
        type: str
        size: 3
        encoding: BCS-N
        doc: |
          Image Display Level
          3 BCS-N positive integer. Display level for CCS positioning.

      - id: IALVL
        type: str
        size: 3
        encoding: BCS-N
        doc: |
          Image Attachment Level
          3 BCS-N positive integer. Attachment level for CCS positioning.

      - id: ILOC
        type: str
        size: 10
        encoding: BCS-N
        doc: |
          Image Location
          10 BCS-N integers. Row and column location on the CCS.
          First 5 digits = row, last 5 digits = column.
          Either integer may be negative.

      - id: NROWS
        type: str
        size: 8
        encoding: BCS-N
        doc: |
          Number of Rows
          8 BCS-N positive integer. Number of rows in the camera image.

      - id: NCOLS
        type: str
        size: 8
        encoding: BCS-N
        doc: |
          Number of Columns
          8 BCS-N positive integer. Number of columns in the camera image.
