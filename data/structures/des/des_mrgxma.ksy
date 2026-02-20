meta:
  id: des_mrgxma
  title: MRGXMA DES User-Defined Subheader
  endian: be

doc: |
  MRGXMA DES - Merged Product XML Data Extension Segment
  
  The MRGXMA DES conveys information about a product that was created by
  combining multiple input images (i.e., a merged product). The DES preserves
  information about the input images used to create the merged product, as
  well as providing information regarding the quality of any applicable
  registration performed on the input images as part of the merging process.
  
  The DES includes both a set of user-defined subheader fields (structured as
  traditional NITF position-based fields) and DESDATA, structured as an XML
  instance document. The user-defined subheader fields specify which image
  segments within the NITF dataset are described by this instance of the
  MRGXMA DES.
  
  The DESSHL is calculated as: 3 + 25 * NUM_SEG_ASSOC
  
  Note: This definition covers the DES-specific subheader fields (DESSHF)
  that appear when DESID is "MRGXMA". The DESDATA field contains an XML
  instance document describing the merge operations and input images.
  
  Reference: STDI-0002 Volume 2, Appendix O - MRGXMA

seq:
  - id: NUM_SEG_ASSOC
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Number of Segments Associated with the DES (NUM_SEG_ASSOC)
      The number of segments in this NITF dataset containing the output
      of the merge operation.
      3 BCS-N characters.
      Range: 001 to 999
      
      For example, if the output of the merge operation is one large image
      that is split into two image segments and one text segment, and the
      data provider chooses to formally associate the text segment with
      the merged output, then the value is 3.

  - id: SEG_ASSOC
    type: seg_assoc_entry
    repeat: expr
    repeat-expr: NUM_SEG_ASSOC.to_i
    doc: |
      Associated segment entries.
      One entry for each associated segment (1 to NUM_SEG_ASSOC).

types:
  seg_assoc_entry:
    seq:
      - id: VALUE
        type: str
        size: 25
        encoding: BCS-A
        doc: |
          Associated Segment (SEG_ASSOCn)
          Identifies a segment associated with this DES.
          25 BCS-A characters.
          
          The format is: {segment_type}{segment_index}
          Where segment_type is one of: IM, GR, TX, DE
          And segment_index is a 3-digit zero-padded number.
          
          Example: "IM001" identifies image segment 1.
