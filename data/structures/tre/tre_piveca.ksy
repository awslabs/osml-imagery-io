meta:
  id: tre_piveca
  title: Pixel Vector TRE (Placeholder)
  endian: be

doc: |
  PIVECA TRE - Pixel Vector Tagged Record Extension (Version A)
  
  NOTE: This TRE specification is currently a placeholder. The PIVECA TRE
  was approved for registration on 2019-02-28, but the full specification
  has not yet been published (marked as "To Be Determined" in STDI-0002).
  
  PIVECA will contain vector information such as:
  - Spectral response
  - Impulse response for SAR
  
  The TRE is associated with a PVIS (Pixel Vector Image Segment).
  
  This definition will be updated when the full specification is published.
  
  Reference: STDI-0002 Volume 1, Appendix AM - PIVECA (Placeholder)

seq:
  - id: data
    size-eos: true
    doc: |
      Raw TRE data.
      The field structure is not yet defined in the specification.
      This placeholder captures the raw bytes for future parsing
      when the specification is finalized.

