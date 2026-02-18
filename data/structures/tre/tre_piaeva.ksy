meta:
  id: tre_piaeva
  title: Profile for Imagery Access Event TRE
  endian: be

doc: |
  PIAEVA TRE - Profile for Imagery Access Event Extension - Version A
  
  Provides information about events identified on an image.
  Present for each event identified, up to 100 per data type.
  
  Reference: STDI-0002 Volume 1, Appendix C - PIAE

seq:
  - id: eventname
    type: str
    size: 38
    encoding: ASCII
    doc: |
      Event Name (EVENTNAME)
      The recognized name of the event.
      38 BCS-A.

  - id: eventtype
    type: str
    size: 8
    encoding: ASCII
    doc: |
      Event Type (EVENTTYPE)
      Generic type of event associated with the product.
      8 BCS-A.
