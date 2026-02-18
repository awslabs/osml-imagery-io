meta:
  id: des_tre_overflow
  title: TRE Overflow DES User-Defined Subheader
  endian: be

doc: |
  TRE_OVERFLOW DES - Tagged Record Extension Overflow Data Extension Segment
  
  This DES is used for encapsulating a series of TREs that overflow from the
  NITF file header or any segment's subheader. A separate DES is used for each
  file header or subheader field that overflows.
  
  Note: This definition covers the DES-specific subheader fields (DESOFLW and
  DESITEM) that appear after the standard DES security fields when DESID is
  "TRE_OVERFLOW". The DESDATA field contains TRE envelopes with no intervening
  bytes.
  
  Reference: STDI-0002 Volume 2, Appendix A - TRE Overflow
  Reference: Joint BIIF Profile (JBP) Section 5.18.4

seq:
  - id: desoflw
    type: str
    size: 6
    encoding: BCS-A
    doc: |
      DES Overflowed Header Type (DESOFLW)
      Indicates the segment type to which the enclosed TREs are relevant.
      6 BCS-A characters.
      Allowed values: XHD, IXSHD, SXSHD, TXSHD, UDHD, UDID
      - UDHD: File header user-defined header data
      - XHD: File header extended header data
      - UDID: Image subheader user-defined image data
      - IXSHD: Image subheader extended subheader data
      - SXSHD: Graphic subheader extended subheader data
      - TXSHD: Text subheader extended subheader data

  - id: desitem
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      DES Data Item Overflowed (DESITEM)
      The 1-based index of the segment to which the TREs apply.
      3 BCS-N positive integer (000-999).
      If DESOFLW is UDHD or XHD, this value is 000.
      Otherwise, indicates which image/graphic/text segment overflowed.

