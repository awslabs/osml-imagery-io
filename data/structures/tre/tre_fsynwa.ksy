meta:
  id: tre_fsynwa
  title: Frame Synchronization Wrapper TRE
  endian: be

doc: |
  FSYNWA TRE - Frame Synchronization Wrapper Tagged Record Extension
  
  Wraps one or more other TREs and associates them to a specific frame
  in a collection. This allows frame-specific metadata to be attached
  to motion imagery data.
  
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
      - Frame reference information
      - Wrapped TRE count
      - Wrapped TRE data
      
      Full field definitions are in NGA.STND.0044_1.3.
