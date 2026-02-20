meta:
  id: des_lidara
  title: LIDARA DES User-Defined Subheader
  endian: be

doc: |
  LIDARA DES - Light Detection and Ranging version A Data Extension Segment
  
  The LIDARA DES methodology is designed to store a LiDAR point cloud dataset,
  in binary Large Area Sensor (LAS) format, in its entirety. The inclusion of
  this DES is optional, but if it is included, it shall be accompanied by an
  Intensity image segment, an Elevation image segment, or both.
  
  Storage of the point cloud data is achieved by carrying out a byte-for-byte
  transfer of the LAS file into the user-defined data portion of the DES. The
  total amount transferred into the DES cannot exceed 999999998 bytes
  (approximately 1 GB). For LAS files larger than 1 GB, multiple instances of
  the LIDARA DES can be used.
  
  The DESSHL for LIDARA is always 0003 bytes.
  
  Note: This definition covers the DES-specific subheader fields (DESSHF)
  that appear when DESID is "LIDARA". The DESDATA field contains the LAS file
  data in its native little-endian byte order.
  
  Reference: STDI-0002 Volume 2, Appendix J - LIDARA
  Reference: LAS Specification, Version 1.3

seq:
  - id: INDES
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      DES Position (INDES)
      The sequential position of the DES with respect to all others created
      to encapsulate an LAS file. This value is assigned during the
      encapsulation process.
      3 BCS-N characters.
      Range: 000 to 998
      Default is 000.
      
      For example, if four instances are needed to encapsulate a given LAS
      file, the INDES values for each one are 0, 1, 2, and 3, respectively.
