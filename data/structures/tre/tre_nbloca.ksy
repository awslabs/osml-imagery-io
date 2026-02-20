meta:
  id: tre_nbloca
  title: NITF Block Offset TRE
  endian: be

doc: |
  NBLOCA TRE - NITF Block Offset Extension
  
  Stores the offsets of each image frame relative to each other within a NITF image.
  The first image frame offset is the number of bytes in the image subheader.
  All other offsets are the number of bytes in the previous image block or frame.
  
  This extension allows the NITF image to be accessed in a random or parallel fashion
  by providing the ability to find the offset to the location of the first data byte
  of any frame or block. For JPEG applications, these offsets are to the Start of
  Image (SOI) markers.
  
  Reference: STDI-0002 Volume 1, Appendix I - NBLOCA

seq:
  - id: FRAME_1_OFFSET
    type: u4
    doc: |
      First Image Frame Offset (FRAME_1_OFFSET)
      Offset from beginning of NITF image subheader.
      4-byte unsigned binary integer, range 439-999999.
      This offset equals the size of the image subheader.

  - id: NUMBER_OF_FRAMES
    type: u4
    doc: |
      Number of Blocks (NUMBER_OF_FRAMES)
      Number of blocks for which offsets are listed.
      4-byte unsigned binary integer, range 1-24996.

  - id: FRAME_OFFSETS
    type: u4
    repeat: expr
    repeat-expr: number_of_frames - 1
    if: NUMBER_OF_FRAMES > 1
    doc: |
      Frame Offsets (FRAME_2_OFFSET to FRAME_N_OFFSET)
      Offset in bytes of the beginning of the nth image frame
      from the beginning of the (N-1) image frame.
      4-byte unsigned binary integer, range 1 to (2^32 - 1).
      For JPEG applications, this is the offset between SOI markers.
