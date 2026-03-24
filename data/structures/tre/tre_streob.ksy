meta:
  id: tre_streob
  title: Stereo Data TRE
  endian: be

doc: |
  STREOB TRE - Stereo Data
  
  Provides stereo pair information including convergence angles,
  asymmetry angles, and BIE (Bisector Intersection Error) values
  for stereo imagery products.
  
  Fixed length: 94 bytes.
  
  Reference: STDI-0002 Volume 1, Appendix E, Section E.3.15, Table E-25

seq:
  - id: ST_ID
    type: str
    size: 60
    encoding: BCS-A
    doc: |
      Stereo ID
      60 BCS-A.

  - id: N_MATES
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Number of Mates
      1 BCS-N integer.

  - id: MATE_INSTANCE
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Mate Instance
      1 BCS-N integer.

  - id: B_CONV
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Beginning Convergence Angle
      5 BCS-N real.

  - id: E_CONV
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Ending Convergence Angle
      5 BCS-N real.

  - id: B_ASYM
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Beginning Asymmetry Angle
      5 BCS-N real.

  - id: E_ASYM
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Ending Asymmetry Angle
      5 BCS-N real.

  - id: B_BIE
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Beginning BIE (Bisector Intersection Error)
      6 BCS-N real.

  - id: E_BIE
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Ending BIE (Bisector Intersection Error)
      6 BCS-N real.
