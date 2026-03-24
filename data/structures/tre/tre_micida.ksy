meta:
  id: tre_micida
  title: Motion Imagery Core Identification TRE
  endian: be

doc: |
  MICIDA TRE - Motion Imagery Core Identification Tagged Record Extension

  Provides a link or association between the camera UUIDs (defined in CAMSDA)
  and the Motion Imagery Identification System (MIIS) core identifier as
  defined by MISB ST 1204. Required in the file header of all files in a
  collection including the manifest file.

  The MICIDA TRE uses the text-based form of the MIIS Core ID (BCS-A string)
  to accommodate the variable-length nature of the identifier.

  Multiple MICIDA TREs may be required if the number of cameras is large.

  This TRE is part of the Motion Imagery Extensions for NITF 2.1 (MIE4NITF)
  specification defined in NGA.STND.0044.

  Reference: STDI-0002 Volume 1, Appendix AF, Section AF 5.3
  Reference: NGA.STND.0044_1.3.3 - Motion Imagery Extension for NITF 2.1
  Reference: MISB ST 1204 - Motion Imagery Identification System

seq:
  - id: MIIS_CORE_ID_VERSION
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      MIIS Core ID Version
      2 BCS-N enumerated value. Version of the MIIS Core ID structure.
      Currently 01.

  - id: NUM_CAMERA_IDS_IN_TRE
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Number of Camera IDs in This TRE
      3 BCS-N positive integer.

  - id: CAMERA_IDS
    type: camera_core_id_record
    repeat: expr
    repeat-expr: NUM_CAMERA_IDS_IN_TRE.to_i
    doc: Camera-to-MIIS-Core-ID mapping records.

types:
  camera_core_id_record:
    seq:
      - id: CAMERA_ID
        type: str
        size: 36
        encoding: BCS-A
        doc: |
          Camera UUID
          36 BCS-A UUID (X.667 format). Must match a CAMERA_ID in CAMSDA.

      - id: CORE_ID_LENGTH
        type: str
        size: 3
        encoding: BCS-N
        doc: |
          Core ID Length
          3 BCS-N positive integer. Length of the MIIS Core ID string
          in bytes.

      - id: CAMERA_CORE_ID
        type: str
        size: CORE_ID_LENGTH.to_i
        encoding: BCS-A
        doc: |
          MIIS Core ID
          Variable-length BCS-A string (length = CORE_ID_LENGTH).
          Text-based MIIS Core ID per MISB ST 1204.
          Uses 4-4-4-4-4-4-4-4 UUID format with colons, dashes,
          and forward slash as separators. Includes version, usage,
          sensor/platform/window UUIDs, and hex checksum.
