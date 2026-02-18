meta:
  id: tre_mimcsa
  title: Motion Imagery Collection Summary TRE
  endian: be

doc: |
  MIMCSA TRE - Motion Imagery Collection Summary Tagged Record Extension
  
  Contains high-level metadata regarding the frame rate range of the motion
  imagery, encoding methods used, and if any temporal subsampling was performed.
  
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
      - Frame rate range information
      - Encoding methods used
      - Temporal subsampling indicators
      
      Full field definitions are in NGA.STND.0044_1.3.
