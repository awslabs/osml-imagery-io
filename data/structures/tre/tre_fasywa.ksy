meta:
  id: tre_fasywa
  title: Frame Asynchronous Wrapper TRE
  endian: be

doc: |
  FASYWA TRE - Frame Asynchronous Wrapper Tagged Record Extension
  
  Wraps one or more other TREs and associates them to a specific point
  in time on the collection timeline. Normally placed in an Image Segment
  Subheader containing data related to the metadata. May be placed in the
  File Header of the file containing the related MI data as well.
  
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
      - Time point reference on collection timeline
      - Wrapped TRE count
      - Wrapped TRE data
      
      Full field definitions are in NGA.STND.0044_1.3.
