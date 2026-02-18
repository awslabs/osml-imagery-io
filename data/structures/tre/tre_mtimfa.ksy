meta:
  id: tre_mtimfa
  title: Motion Imagery Temporal Block Mapping TRE
  endian: be

doc: |
  MTIMFA TRE - Motion Imagery Temporal Block Mapping Tagged Record Extension
  
  Specifies how the MI data for all cameras in a phenomenological layer for
  a given camera set and time interval are subdivided into temporal blocks.
  Also associates the temporal blocks to the Image Segment index.
  
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
      - Camera set reference
      - Time interval reference
      - Phenomenological layer reference
      - Temporal block definitions
      - Image segment index associations
      
      Full field definitions are in NGA.STND.0044_1.3.
