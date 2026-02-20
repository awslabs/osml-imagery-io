meta:
  id: tre_freesa
  title: Free Space TRE
  endian: be

doc: |
  FREESA TRE - Free Space Tagged Record Extension
  
  Saves space for metadata that may not yet be available when a file is
  written out or used when actual metadata size does not match predicted
  metadata size. May be removed at any time.
  
  IMPORTANT: No data may be placed in this TRE. The TRE exists solely as
  a placeholder to reserve space in the file structure.
  
  This TRE is part of the Motion Imagery Extensions for NITF 2.1 (MIE4NITF)
  specification defined in NGA.STND.0044.
  
  Note: This is a stub definition. The full field specifications are defined
  in NGA.STND.0044_1.3 which is not publicly available in STDI-0002.
  The raw data is preserved for round-trip fidelity.
  
  Reference: STDI-0002 Volume 1, Appendix AF - MIE4NITF
  Reference: NGA.STND.0044_1.3 - Motion Imagery Extension for NITF 2.1

seq:
  - id: DATA
    size-eos: true
    doc: |
      Reserved space (should be empty or filled with padding).
      
      This TRE is a placeholder only - no meaningful data should be present.
      
      Full field definitions are in NGA.STND.0044_1.3.
