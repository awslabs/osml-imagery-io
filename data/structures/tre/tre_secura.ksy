meta:
  id: tre_secura
  title: Extended Security Marking Metadata TRE
  endian: be

doc: |
  SECURA TRE - Extended Security Marking Metadata Tagged Record Extension
  
  Provides additional security marking metadata for NITF 2.0 and 2.1 files
  using ODNI Intelligence Community Technical Specification, XML Data
  Encoding Specification for Access Rights and Handling (ARH.XML).
  
  The TRE contains:
  - Copy of FDT field from file header for validity check
  - Copy of security fields from header/subheader for validity check
  - Security standard identifier
  - Compression flag
  - Security data (XML or compressed XML)
  
  Can be placed in file header (applies to entire file) or segment
  subheader (applies to specific segment).
  
  Variable length TRE (minimum 251 bytes)
  
  Reference: STDI-0002 Volume 1, Appendix AI - SECURA

seq:
  - id: FDATTIM
    type: str
    size: 14
    encoding: BCS-A
    doc: |
      NITF Date Time Field (FDT).
      Byte copy of the associated FDT field in the NITF file.
      Format: CCYYMMDDhhmmss (NITF 2.1) or DDHHMMSSZmonYY (NITF 2.0).
      Used for validity check between TRE and file header.

  - id: NITFVER
    type: str
    size: 9
    encoding: BCS-A
    doc: |
      NITF Version Flag.
      Values: "NITF02.00" or "NITF02.10"

  - id: NFSECFLDS
    size: 207
    doc: |
      NITF Security Fields (FSEC).
      Byte copy of the associated segment's security fields.
      
      For NITF 2.1: 167 bytes from security fields followed by
      40 bytes of zero fill (0x00).
      
      For NITF 2.0: 207 bytes from security fields (xSCLAS,
      xSCODE, xSCTLH, xSREL, xSCAUT, xSCTLN, xSDWNG, xSDEVT).
      
      Used for validity check between TRE and header/subheader.

  - id: SECSTD
    type: str
    size: 8
    encoding: BCS-A
    doc: |
      Security Standard.
      The security standard used to populate the SECURITY field.
      Left justified, blank filled.
      Value: "ARH.XML " for ODNI Intelligence Community Technical
      Specification, XML Data Encoding Specification for Access
      Rights and Handling.

  - id: SECCOMP
    type: str
    size: 8
    encoding: BCS-A
    doc: |
      SECURITY Field Compression.
      Specifies compression type of the SECURITY field.
      Left justified, blank filled.
      Values:
      - Spaces (8 spaces): Field is uncompressed
      - "GZIP    ": Field is compressed with GZIP per IETF RFC-1952

  - id: SECLEN
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      SECURITY Length.
      Length in bytes of the SECURITY field.
      If compressed, this is the length of the compressed data stream,
      not the length of the original security marking data.
      Range: 00000-99737.

  - id: SECURITY
    size: SECLEN.to_i
    doc: |
      Security Data.
      The actual security data, encoded using the security standard
      specified in SECSTD and compressed as specified by SECCOMP.
      
      If SECSTD is "ARH.XML", this field (prior to any compression)
      must be a UTF-8 encoded XML instance document compliant with
      XML 1.1, where the root of the document is the ARH Security
      element specifying the information security marking metadata.
      
      Note: UTF-8 characters can be encoded using more than one byte,
      so the number of bytes is not necessarily equal to the number
      of characters in the XML instance document.

