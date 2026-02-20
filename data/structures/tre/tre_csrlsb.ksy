meta:
  id: tre_csrlsb
  title: Common Sensor Rolling Shutter Terms TRE
  endian: be

doc: |
  CSRLSB TRE - Common Sensor Rolling Shutter Terms
  Version 1.2
  
  Part of the GLAS/GFM (Generic Linear Array Scanner / Generic Frame-sequence Model)
  support data extensions. Provides time as a function of pixel location across a frame
  for rolling shutter sensors. Time differences (delta-T) with respect to the frame's
  reference time are modeled by blocks such that each corner of the block is assigned
  a DT value.
  
  This TRE is used in conjunction with CSEXRB TRE and CSATTB/CSEPHB DESs to interpolate
  ephemeris and attitude data for rolling shutter frame sensors.
  
  Reference: STDI-0002 Volume 2, Appendix M - GLAS-GFM

seq:
  - id: N_RS_ROW_BLOCKS
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Blocks in Row Dimension
      Number of equally spaced blocks in the row dimension of the image.
      2 BCS-N integer, range 01-99.

  - id: M_RS_COLUMN_BLOCKS
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Blocks in Column Dimension
      Number of equally spaced blocks in the column dimension of the image.
      2 BCS-N integer, range 01-99.

  - id: RS_BLOCKS
    type: rs_block_t
    repeat: expr
    repeat-expr: N_RS_ROW_BLOCKS.to_i * M_RS_COLUMN_BLOCKS.to_i
    doc: |
      Rolling Shutter Block Data
      Array of blocks containing delta time values for each corner.
      Total blocks = N_RS_ROW_BLOCKS × M_RS_COLUMN_BLOCKS.

types:
  rs_block_t:
    doc: |
      Rolling Shutter Block
      Contains delta time values for the 4 corners of a block.
    seq:
      - id: RS_DT_1
        type: str
        size: 12
        encoding: BCS-N
        doc: |
          Rolling Shutter Delta Time 1 (Upper-left corner)
          Delta time with respect to the image reference time.
          12 BCS-N real number, range -9999999999 to +9999999999 milliseconds.

      - id: RS_DT_2
        type: str
        size: 12
        encoding: BCS-N
        doc: |
          Rolling Shutter Delta Time 2 (Upper-right corner)
          Delta time with respect to the image reference time.
          12 BCS-N real number, range -9999999999 to +9999999999 milliseconds.

      - id: RS_DT_3
        type: str
        size: 12
        encoding: BCS-N
        doc: |
          Rolling Shutter Delta Time 3 (Lower-right corner)
          Delta time with respect to the image reference time.
          12 BCS-N real number, range -9999999999 to +9999999999 milliseconds.

      - id: RS_DT_4
        type: str
        size: 12
        encoding: BCS-N
        doc: |
          Rolling Shutter Delta Time 4 (Lower-left corner)
          Delta time with respect to the image reference time.
          12 BCS-N real number, range -9999999999 to +9999999999 milliseconds.
