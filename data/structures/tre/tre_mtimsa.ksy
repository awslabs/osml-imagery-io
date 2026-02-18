meta:
  id: tre_mtimsa
  title: Motion Imagery Timing TRE
  endian: be

doc: |
  MTIMSA TRE - Motion Imagery Timing Tagged Record Extension
  
  Specifies the nominal frame rate, frame numbers and time stamps for the
  MI data within the Image Segment in which the TRE is found. Ties this
  information back to the phenomenological layer, camera set, camera,
  time interval and temporal block associated with the Image Segment.
  
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
      - Nominal frame rate
      - Frame numbers
      - Time stamps
      - Phenomenological layer reference
      - Camera set reference
      - Camera reference
      - Time interval reference
      - Temporal block reference
      
      Full field definitions are in NGA.STND.0044_1.3.
