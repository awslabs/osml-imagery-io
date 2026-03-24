meta:
  id: tre_freesa
  title: Free Space TRE
  endian: be

doc: |
  FREESA TRE - Free Space Tagged Record Extension

  Saves space for metadata that may not yet be available when a file is
  written out or used when actual metadata size does not match predicted
  metadata size. May be removed at any time by any system provided that
  the integrity of the NITF file is maintained.

  IMPORTANT: No meaningful data may be placed in this TRE. The STUFFING
  field must contain only 0xFF bytes.

  This TRE is part of the Motion Imagery Extensions for NITF 2.1 (MIE4NITF)
  specification defined in NGA.STND.0044.

  Reference: STDI-0002 Volume 1, Appendix AF, Section AF 5.6
  Reference: NGA.STND.0044_1.3.3 - Motion Imagery Extension for NITF 2.1

seq:
  - id: STUFFING
    size-eos: true
    doc: |
      Stuffing Bytes
      Variable length. Must contain only 0xFF bytes.
      This field exists solely to reserve space in the file structure.
