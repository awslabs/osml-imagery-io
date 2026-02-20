meta:
  id: tre_camsda
  title: Camera Set Definition TRE
  endian: be

doc: |
  CAMSDA TRE - Camera Set Definition Tagged Record Extension
  
  Defines the camera sets, places cameras on the NCCS (NITF Common Coordinate
  System), assigns phenomenological layer IDs and UUIDs to all cameras in
  the collection.
  
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
      Raw TRE data.
      
      Expected content includes:
      - Camera set definitions
      - Camera positions on NCCS
      - Phenomenological layer IDs
      - Camera UUIDs
      
      Full field definitions are in NGA.STND.0044_1.3.
