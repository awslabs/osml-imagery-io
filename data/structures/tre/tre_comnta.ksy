meta:
  id: tre_comnta
  title: Comments TRE
  endian: be

doc: |
  COMNTA TRE - Comments Tagged Record Extension
  
  Allows data providers to include unstructured textual comments in the
  file header, image segment, graphic segment, or text segment. The comment
  is encoded using UTF-8 characters, allowing for full Unicode support
  including multi-byte characters.
  
  If the comment is to be formatted as multiple lines, carriage return
  followed by line feed (0x0d, 0x0a) is inserted between lines.
  
  The Unicode Byte Order Mark (BOM) must be omitted from the encoded
  UTF-8 byte sequence.
  
  Reference: STDI-0002 Volume 1, Appendix AU - COMNTA

seq:
  - id: COMMENT
    type: str
    size-eos: true
    encoding: UTF-8
    doc: |
      Comment - Unstructured textual comment encoded using UTF-8 characters.
      The length is determined by the CEL field (TREL) in the TRE envelope.
      
      If the comment is to be displayed as multiple lines, carriage return
      and line feed (0x0d, 0x0a) character pairs are inserted between lines.
      
      If the comment is classified, it starts with a security portion mark
      including codeword(s).
      
      Note: If the comment includes multi-byte characters, the number of
      characters will be less than the byte length (CEL/TREL).
