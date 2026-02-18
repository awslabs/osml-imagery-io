meta:
  id: tre_mitoca
  title: Multi-image Scene Table of Contents TRE
  endian: be

doc: |
  MITOCA TRE - Multi-image Scene (MiS) Table of Contents Tagged Record Extension
  
  Provides a mechanism for managing multi-image scenes collected over a designated
  coverage area. The MITOCA TRE allows a collection of images to be treated as a
  single image scene, supporting sensors that collect multiple images to cover a
  full scene or image the same footprint multiple times (Looks).
  
  The TRE contains three main sections:
  - MiS Section: Scene-level information (SCENE_TYPE through NUM_VOLUMES)
  - Volume Section: Volume-level information (LOOK_INSTANCE through DSR)
  - Component Section: Per-component image information (repeats NUM_COMPONENTS times)
  
  Variable length TRE with conditional fields based on LOOK_COMPOSITE_INDEX,
  LOOK_COMPOSITE_ID_LEN, COMPONENT_INDEX_TYPE, and VOLUME_COMPOSITE_INDEX values.
  
  Reference: STDI-0002 Volume 1, Appendix O - MITOCA

seq:
  # ============================================
  # MiS Section (Scene-level fields)
  # ============================================
  - id: scene_type
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Scene Type. Identifies the implementation practices and conventions.
      000 = Not designated
      001-100 = DCGS Reserved
      101-999 = Reserved for future user communities

  - id: scene_id_len
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Scene Identifier Length.
      Length of the SCENE_ID field in bytes (001-999).

  - id: scene_id
    type: str
    size: scene_id_len.to_i
    encoding: BCS-A
    doc: |
      Scene Identifier.
      Common identifier for all components of a scene.
      Variable length (1-999 characters).

  - id: look_composite_index
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Look Composite Index Value.
      000 = Look Composite not present but MBP4 coordinates populated
      001-999 = IDLVL value of Look Composite image
      --- = Look Composite not present in this NITF file

  - id: look_composite_id_len
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Look Composite Identifier Length.
      Length of LOOK_COMPOSITE_ID field (000-999 bytes).
      000 if LOOK_COMPOSITE_INDEX is "---".

  - id: look_composite_id
    type: str
    size: look_composite_id_len.to_i
    encoding: BCS-A
    if: look_composite_id_len.to_i > 0
    doc: |
      Look Composite Image Identifier.
      Present only if LOOK_COMPOSITE_ID_LEN is not 000.

  # Look MBP4 coordinates (conditional - not present if LOOK_COMPOSITE_INDEX is "---")
  - id: look_corner_1
    type: str
    size: 21
    encoding: BCS-A
    if: look_composite_index != "---"
    doc: |
      Look Corner Point 1 - Pixel (0,0).
      Format: XDDMMSS.SSYDDMMSS.SS or dd.ddddddddd.dddddd

  - id: look_corner_2
    type: str
    size: 21
    encoding: BCS-A
    if: look_composite_index != "---"
    doc: |
      Look Corner Point 2 - Pixel (0, MaxCol).
      Format: XDDMMSS.SSYDDMMSS.SS or dd.ddddddddd.dddddd

  - id: look_corner_3
    type: str
    size: 21
    encoding: BCS-A
    if: look_composite_index != "---"
    doc: |
      Look Corner Point 3 - Pixel (MaxRow, MaxCol).
      Format: XDDMMSS.SSYDDMMSS.SS or dd.ddddddddd.dddddd

  - id: look_corner_4
    type: str
    size: 21
    encoding: BCS-A
    if: look_composite_index != "---"
    doc: |
      Look Corner Point 4 - Pixel (MaxRow, 0).
      Format: XDDMMSS.SSYDDMMSS.SS or dd.ddddddddd.dddddd

  - id: num_volumes
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      Number of Volumes.
      Total number of volumes that make up the Look.
      000001-999999 or "------" for unknown/default.

  # ============================================
  # Volume Section
  # ============================================
  - id: look_instance
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Look Instance.
      Identifies which Look the data in this volume belongs to (000001-999999).

  - id: volume_num
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Volume Number.
      Identifies which volume of the overall MiS (000001-999999).
      Recommended to be numbered in temporal order.

  - id: sensor_id
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      Sensor Identifier.
      Identifies which specific sensor produced the images in this volume.

  - id: sensor_id_type
    type: str
    size: 4
    encoding: BCS-A
    doc: |
      Sensor Identifier Type.
      Identifies the type of sensor that produced the images.

  - id: mplan
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Mission Plan Mode.
      Identifies the collection (imaging) mode of the sensor.

  - id: volume_composite_index
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Volume Composite Image Index Value.
      000 = Volume composite not present in this NITF file
      001-999 = Display level (IDLVL) of composite image segment

  - id: volume_composite_id_len
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Volume Composite Identifier Length.
      Length of VOLUME_COMPOSITE_ID field (001-999 bytes).

  - id: volume_composite_id
    type: str
    size: volume_composite_id_len.to_i
    encoding: BCS-A
    doc: |
      Volume Composite Image Identifier.
      Portion of the Volume Composite image ID that uniquely identifies it.

  # Volume MBP4 coordinates (always present)
  - id: volume_corner_1
    type: str
    size: 21
    encoding: BCS-A
    doc: |
      Volume Corner Point 1 - Pixel (0,0).
      Format: XDDMMSS.SSYDDMMSS.SS or dd.ddddddddd.dddddd

  - id: volume_corner_2
    type: str
    size: 21
    encoding: BCS-A
    doc: |
      Volume Corner Point 2 - Pixel (0, MaxCol).
      Format: XDDMMSS.SSYDDMMSS.SS or dd.ddddddddd.dddddd

  - id: volume_corner_3
    type: str
    size: 21
    encoding: BCS-A
    doc: |
      Volume Corner Point 3 - Pixel (MaxRow, MaxCol).
      Format: XDDMMSS.SSYDDMMSS.SS or dd.ddddddddd.dddddd

  - id: volume_corner_4
    type: str
    size: 21
    encoding: BCS-A
    doc: |
      Volume Corner Point 4 - Pixel (MaxRow, 0).
      Format: XDDMMSS.SSYDDMMSS.SS or dd.ddddddddd.dddddd

  - id: num_components
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Number of Component Image Segments.
      Number of component images in this Volume (001-999).

  - id: components_flag
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Component Image Segments Presence Flag.
      0 = None of the component images are in this NITF file
      1 = All component images are in this NITF file
      2-9 = Reserved

  - id: num_rows
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Number of Pixel Rows.
      Number of significant pixel rows in the Volume composite image.
      Same as NROWS in composite image subheader (00000001-99999999).

  - id: num_cols
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Number of Pixel Columns.
      Number of significant pixel columns in the Volume composite image.
      Same as NCOLS in composite image subheader (00000001-99999999).

  - id: dsr
    type: str
    size: 7
    encoding: BCS-N
    doc: |
      Down Sample Ratio.
      Down sample ratio of the Volume composite image (0001.00-9999.99).
      0001.00 = no downsample, 0002.00 = 2:1, etc.

  # ============================================
  # Component Section (repeats NUM_COMPONENTS times)
  # ============================================
  - id: component_id_len
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Component Identifier Length.
      Length of COMPONENT_ID field (001-999 bytes).

  - id: component_index_type
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Component Image Index Type.
      0 = ISH_INDEX field is omitted
      1 = ISH_INDEX contains Display Level
      2 = ISH_INDEX contains Sequence Number
      3-9 = Reserved

  - id: components
    type: component_entry
    repeat: expr
    repeat-expr: num_components.to_i
    doc: Per-component image parameters.

types:
  component_entry:
    doc: |
      Component image entry. Contains identification, geo-coordinates,
      and optional pixel offsets for each component image.
    seq:
      - id: component_id
        type: str
        size: _root.component_id_len.to_i
        encoding: BCS-A
        doc: |
          Component Image ID.
          Portion of the component image ID that uniquely identifies it
          within the Look (1-999 characters).

      - id: ish_index
        type: str
        size: 3
        encoding: BCS-N
        if: _root.component_index_type.to_i != 0
        doc: |
          Image Sub-Header Index.
          Display Level or sequence count of the Component Image Segment.
          Present only if COMPONENT_INDEX_TYPE is not 0.

      # Component MBP4 coordinates (always present)
      - id: component_corner_1
        type: str
        size: 21
        encoding: BCS-A
        doc: |
          Component Corner Point 1 - Pixel (0,0).
          Format: XDDMMSS.SSYDDMMSS.SS or dd.ddddddddd.dddddd

      - id: component_corner_2
        type: str
        size: 21
        encoding: BCS-A
        doc: |
          Component Corner Point 2 - Pixel (0, MaxCol).
          Format: XDDMMSS.SSYDDMMSS.SS or dd.ddddddddd.dddddd

      - id: component_corner_3
        type: str
        size: 21
        encoding: BCS-A
        doc: |
          Component Corner Point 3 - Pixel (MaxRow, MaxCol).
          Format: XDDMMSS.SSYDDMMSS.SS or dd.ddddddddd.dddddd

      - id: component_corner_4
        type: str
        size: 21
        encoding: BCS-A
        doc: |
          Component Corner Point 4 - Pixel (MaxRow, 0).
          Format: XDDMMSS.SSYDDMMSS.SS or dd.ddddddddd.dddddd

      # Pixel offsets (conditional - not present if VOLUME_COMPOSITE_INDEX = 0)
      - id: upper_left_row
        type: str
        size: 8
        encoding: BCS-N
        if: _root.volume_composite_index.to_i != 0
        doc: |
          Upper Left Row Pixel Offset.
          Row pixel offset of upper left corner relative to Volume composite.

      - id: upper_left_col
        type: str
        size: 8
        encoding: BCS-N
        if: _root.volume_composite_index.to_i != 0
        doc: |
          Upper Left Column Pixel Offset.
          Column pixel offset of upper left corner relative to Volume composite.

      - id: upper_right_row
        type: str
        size: 8
        encoding: BCS-N
        if: _root.volume_composite_index.to_i != 0
        doc: |
          Upper Right Row Pixel Offset.
          Row pixel offset of upper right corner relative to Volume composite.

      - id: upper_right_col
        type: str
        size: 8
        encoding: BCS-N
        if: _root.volume_composite_index.to_i != 0
        doc: |
          Upper Right Column Pixel Offset.
          Column pixel offset of upper right corner relative to Volume composite.

      - id: lower_right_row
        type: str
        size: 8
        encoding: BCS-N
        if: _root.volume_composite_index.to_i != 0
        doc: |
          Lower Right Row Pixel Offset.
          Row pixel offset of lower right corner relative to Volume composite.

      - id: lower_right_col
        type: str
        size: 8
        encoding: BCS-N
        if: _root.volume_composite_index.to_i != 0
        doc: |
          Lower Right Column Pixel Offset.
          Column pixel offset of lower right corner relative to Volume composite.

      - id: lower_left_row
        type: str
        size: 8
        encoding: BCS-N
        if: _root.volume_composite_index.to_i != 0
        doc: |
          Lower Left Row Pixel Offset.
          Row pixel offset of lower left corner relative to Volume composite.

      - id: lower_left_col
        type: str
        size: 8
        encoding: BCS-N
        if: _root.volume_composite_index.to_i != 0
        doc: |
          Lower Left Column Pixel Offset.
          Column pixel offset of lower left corner relative to Volume composite.
