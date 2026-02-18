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
  - id: n_rs_row_blocks
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Blocks in Row Dimension
      Number of equally spaced blocks in the row dimension of the image.
      2 BCS-N integer, range 01-99.

  - id: m_rs_column_blocks
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Blocks in Column Dimension
      Number of equally spaced blocks in the column dimension of the image.
      2 BCS-N integer, range 01-99.

  - id: rs_blocks
    type: rs_block_t
    repeat: expr
    repeat-expr: n_rs_row_blocks.to_i * m_rs_column_blocks.to_i
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
      - id: rs_dt_1
        type: str
        size: 12
        encoding: BCS-N
        doc: |
          Rolling Shutter Delta Time 1 (Upper-left corner)
          Delta time with respect to the image reference time.
          12 BCS-N real number, range -9999999999 to +9999999999 milliseconds.

      - id: rs_dt_2
        type: str
        size: 12
        encoding: BCS-N
        doc: |
          Rolling Shutter Delta Time 2 (Upper-right corner)
          Delta time with respect to the image reference time.
          12 BCS-N real number, range -9999999999 to +9999999999 milliseconds.

      - id: rs_dt_3
        type: str
        size: 12
        encoding: BCS-N
        doc: |
          Rolling Shutter Delta Time 3 (Lower-right corner)
          Delta time with respect to the image reference time.
          12 BCS-N real number, range -9999999999 to +9999999999 milliseconds.

      - id: rs_dt_4
        type: str
        size: 12
        encoding: BCS-N
        doc: |
          Rolling Shutter Delta Time 4 (Lower-left corner)
          Delta time with respect to the image reference time.
          12 BCS-N real number, range -9999999999 to +9999999999 milliseconds.
