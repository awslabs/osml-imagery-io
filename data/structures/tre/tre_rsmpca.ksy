meta:
  id: tre_rsmpca
  title: RSM Polynomial Coefficients TRE
  endian: be

doc: |
  RSMPCA TRE - Replacement Sensor Model Polynomial Coefficients
  
  Provides polynomial coefficients for a single image section of the RSM.
  Contains row and column section numbers, fit error, normalization
  parameters, and polynomial coefficients for row numerator, row
  denominator, column numerator, and column denominator.
  
  CEL: 486-18546 bytes (variable based on number of polynomial terms)
  
  Reference: STDI-0002 Volume 1, Appendix U - RSM

seq:
  - id: IID
    type: str
    size: 80
    encoding: BCS-A
    doc: |
      Image Identifier
      80 BCS-A characters identifying the image.

  - id: EDITION
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      RSM Image Support Data Edition
      40 BCS-A characters identifying the edition.

  - id: RSN
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Row Section Number
      3 BCS-NPI positive integer (1 to RNIS).

  - id: CSN
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Column Section Number
      3 BCS-NPI positive integer (1 to CNIS).

  - id: RFEP
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row Fit Error in Pixels
      21 BCS-N real number.

  - id: CFEP
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column Fit Error in Pixels
      21 BCS-N real number.

  - id: RNRMO
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row Normalization Offset
      21 BCS-N real number.

  - id: CNRMO
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column Normalization Offset
      21 BCS-N real number.

  - id: XNRMO
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      X Normalization Offset
      21 BCS-N real number.

  - id: YNRMO
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Y Normalization Offset
      21 BCS-N real number.

  - id: ZNRMO
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Z Normalization Offset
      21 BCS-N real number.

  - id: RNRMSF
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row Normalization Scale Factor
      21 BCS-N real number.

  - id: CNRMSF
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column Normalization Scale Factor
      21 BCS-N real number.

  - id: XNRMSF
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      X Normalization Scale Factor
      21 BCS-N real number.

  - id: YNRMSF
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Y Normalization Scale Factor
      21 BCS-N real number.

  - id: ZNRMSF
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Z Normalization Scale Factor
      21 BCS-N real number.

  - id: RNPWRX
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Row Numerator Maximum Power of X
      1 BCS-NPI digit (0-9).

  - id: RNPWRY
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Row Numerator Maximum Power of Y
      1 BCS-NPI digit (0-9).

  - id: RNPWRZ
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Row Numerator Maximum Power of Z
      1 BCS-NPI digit (0-9).

  - id: RNTRMS
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Row Numerator Polynomial Terms
      3 BCS-NPI positive integer.

  - id: RNPCF
    type: str
    size: 21
    encoding: BCS-N
    repeat: expr
    repeat-expr: RNTRMS.to_i
    doc: |
      Row Numerator Polynomial Coefficients
      RNTRMS coefficients, each 21 BCS-N real number.

  - id: RDPWRX
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Row Denominator Maximum Power of X
      1 BCS-NPI digit (0-9).

  - id: RDPWRY
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Row Denominator Maximum Power of Y
      1 BCS-NPI digit (0-9).

  - id: RDPWRZ
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Row Denominator Maximum Power of Z
      1 BCS-NPI digit (0-9).

  - id: RDTRMS
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Row Denominator Polynomial Terms
      3 BCS-NPI positive integer.

  - id: RDPCF
    type: str
    size: 21
    encoding: BCS-N
    repeat: expr
    repeat-expr: RDTRMS.to_i
    doc: |
      Row Denominator Polynomial Coefficients
      RDTRMS coefficients, each 21 BCS-N real number.

  - id: CNPWRX
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Column Numerator Maximum Power of X
      1 BCS-NPI digit (0-9).

  - id: CNPWRY
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Column Numerator Maximum Power of Y
      1 BCS-NPI digit (0-9).

  - id: CNPWRZ
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Column Numerator Maximum Power of Z
      1 BCS-NPI digit (0-9).

  - id: CNTRMS
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Column Numerator Polynomial Terms
      3 BCS-NPI positive integer.

  - id: CNPCF
    type: str
    size: 21
    encoding: BCS-N
    repeat: expr
    repeat-expr: CNTRMS.to_i
    doc: |
      Column Numerator Polynomial Coefficients
      CNTRMS coefficients, each 21 BCS-N real number.

  - id: CDPWRX
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Column Denominator Maximum Power of X
      1 BCS-NPI digit (0-9).

  - id: CDPWRY
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Column Denominator Maximum Power of Y
      1 BCS-NPI digit (0-9).

  - id: CDPWRZ
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Column Denominator Maximum Power of Z
      1 BCS-NPI digit (0-9).

  - id: CDTRMS
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Column Denominator Polynomial Terms
      3 BCS-NPI positive integer.

  - id: CDPCF
    type: str
    size: 21
    encoding: BCS-N
    repeat: expr
    repeat-expr: CDTRMS.to_i
    doc: |
      Column Denominator Polynomial Coefficients
      CDTRMS coefficients, each 21 BCS-N real number.
