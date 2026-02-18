meta:
  id: tre_micida
  title: Motion Imagery Collection ID TRE
  endian: be

doc: |
  MICIDA TRE - Motion Imagery Collection ID Tagged Record Extension
  
  The MICIDA TRE is required in the file header of all files in a collection
  including the manifest file. It provides identification information for
  the motion imagery collection.
  
  This TRE is part of the Motion Imagery Extensions for NITF 2.1 (MIE4NITF)
  specification defined in NGA.STND.0044.
  
  Note: This is a stub definition. The full field specifications are defined
  in NGA.STND.0044_1.3 which is not publicly available in STDI-0002.
  The raw data is preserved for round-trip fidelity.
  
  Reference: STDI-0002 Volume 1, Appendix AF - MIE4NITF
  Reference: NGA.STND.0044_1.3 - Motion Imagery Extension for NITF 2.1

seq:
  - id: data
    size-eos: true
    doc: |
      Raw TRE data.
      
      Expected content includes:
      - Collection identification
      - File identification within collection
      
      Full field definitions are in NGA.STND.0044_1.3.
