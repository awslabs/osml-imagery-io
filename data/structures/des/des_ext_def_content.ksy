meta:
  id: des_ext_def_content
  title: EXT_DEF_CONTENT DES User-Defined Subheader
  endian: be

doc: |
  EXT_DEF_CONTENT DES - Externally Defined Content Data Extension Segment
  
  This DES allows for arbitrary content or data to be embedded in a NITF File.
  Its intended purpose is to allow for the embedding of content where the format
  of that content is clearly and unambiguously defined in an external (to JBP)
  standard and that data either cannot be stored using standard NITF segments
  or that data is semantically supplemental to the essence of the NITF file.
  
  Example types of content include Microsoft Word, PDF files, or standalone
  AVI or MPEG4 video files.
  
  Information about the content, how it is stored and encoded, and how it can
  be used are stored in a block of HTTP/1.1 style headers, in the format
  specified by IETF RFC 7231.
  
  The DESSHL range is 0005 to 9798 bytes.
  
  Note: This definition covers the DES-specific subheader fields (DESSHF)
  that appear when DESID is "EXT_DEF_CONTENT". The DESDATA field contains
  the encoded content.
  
  Reference: STDI-0002 Volume 2, Appendix K - EXT_DEF_CONTENT

seq:
  - id: CONTENT_HEADERS_LEN
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Length in bytes of the CONTENT_HEADERS field (CONTENT_HEADERS_LEN)
      4 BCS-N characters.
      Unsigned integer value up to 9794.

  - id: CONTENT_HEADERS
    type: str
    size: CONTENT_HEADERS_LEN.to_i
    encoding: ECS-A
    doc: |
      Metadata describing the embedded content (CONTENT_HEADERS)
      Key/value pairs structured according to the header definition in
      IETF RFC 7231.
      
      The field is encoded in the format defined for content delivery headers
      in the HTTP/1.1 standard. This attribute list may contain any valid
      content delivery header defined in IETF RFC 7231, defined extensions
      to that standard, or defined directly within the specification.
      
      Required headers:
      - Content-Type: Type or format of the embedded data (IETF RFC 6838)
      - Content-Use: Use or meaning of the content
      
      Optional headers include:
      - Content-Encoding: Encoding applied to the content (e.g., gzip)
      - Content-Range: Byte range if content is split across DES instances
      - Content-Description: Human-readable description
      - Content-Disposition: Filename and creation date
      - Content-Length: Size of the encoded content in bytes
      - Canonical-ID: UUID or other canonical identifier
      - DES-ID1, DES-ID2: Additional identifiers
      - Associated-Location-Point: Geographic point
      - Associated-Location-Polygon: Geographic polygon
      - Associated-IDLVL: Associated image display levels
      
      Non-ECS characters within content headers shall be encoded using
      IETF RFC 2047, using the UTF-8 character set.
