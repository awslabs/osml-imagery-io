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
  - id: iid
    type: str
    size: 80
    encoding: BCS-A
    doc: |
      Image Identifier
      80 BCS-A characters identifying the image.

  - id: edition
    type: str
    size: 40
    encoding: BCS-A
    doc: |
      RSM Image Support Data Edition
      40 BCS-A characters identifying the edition.

  - id: rsn
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Row Section Number
      3 BCS-NPI positive integer (1 to RNIS).

  - id: csn
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Column Section Number
      3 BCS-NPI positive integer (1 to CNIS).

  - id: rfep
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row Fit Error in Pixels
      21 BCS-N real number.

  - id: cfep
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column Fit Error in Pixels
      21 BCS-N real number.

  - id: rnrmo
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row Normalization Offset
      21 BCS-N real number.

  - id: cnrmo
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column Normalization Offset
      21 BCS-N real number.

  - id: xnrmo
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      X Normalization Offset
      21 BCS-N real number.

  - id: ynrmo
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Y Normalization Offset
      21 BCS-N real number.

  - id: znrmo
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Z Normalization Offset
      21 BCS-N real number.

  - id: rnrmsf
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Row Normalization Scale Factor
      21 BCS-N real number.

  - id: cnrmsf
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Column Normalization Scale Factor
      21 BCS-N real number.

  - id: xnrmsf
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      X Normalization Scale Factor
      21 BCS-N real number.

  - id: ynrmsf
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Y Normalization Scale Factor
      21 BCS-N real number.

  - id: znrmsf
    type: str
    size: 21
    encoding: BCS-N
    doc: |
      Z Normalization Scale Factor
      21 BCS-N real number.

  - id: rnpwrx
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Row Numerator Maximum Power of X
      1 BCS-NPI digit (0-9).

  - id: rnpwry
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Row Numerator Maximum Power of Y
      1 BCS-NPI digit (0-9).

  - id: rnpwrz
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Row Numerator Maximum Power of Z
      1 BCS-NPI digit (0-9).

  - id: rntrms
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Row Numerator Polynomial Terms
      3 BCS-NPI positive integer.

  - id: rnpcf
    type: str
    size: 21
    encoding: BCS-N
    repeat: expr
    repeat-expr: rntrms.to_i
    doc: |
      Row Numerator Polynomial Coefficients
      RNTRMS coefficients, each 21 BCS-N real number.

  - id: rdpwrx
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Row Denominator Maximum Power of X
      1 BCS-NPI digit (0-9).

  - id: rdpwry
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Row Denominator Maximum Power of Y
      1 BCS-NPI digit (0-9).

  - id: rdpwrz
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Row Denominator Maximum Power of Z
      1 BCS-NPI digit (0-9).

  - id: rdtrms
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Row Denominator Polynomial Terms
      3 BCS-NPI positive integer.

  - id: rdpcf
    type: str
    size: 21
    encoding: BCS-N
    repeat: expr
    repeat-expr: rdtrms.to_i
    doc: |
      Row Denominator Polynomial Coefficients
      RDTRMS coefficients, each 21 BCS-N real number.

  - id: cnpwrx
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Column Numerator Maximum Power of X
      1 BCS-NPI digit (0-9).

  - id: cnpwry
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Column Numerator Maximum Power of Y
      1 BCS-NPI digit (0-9).

  - id: cnpwrz
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Column Numerator Maximum Power of Z
      1 BCS-NPI digit (0-9).

  - id: cntrms
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Column Numerator Polynomial Terms
      3 BCS-NPI positive integer.

  - id: cnpcf
    type: str
    size: 21
    encoding: BCS-N
    repeat: expr
    repeat-expr: cntrms.to_i
    doc: |
      Column Numerator Polynomial Coefficients
      CNTRMS coefficients, each 21 BCS-N real number.

  - id: cdpwrx
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Column Denominator Maximum Power of X
      1 BCS-NPI digit (0-9).

  - id: cdpwry
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Column Denominator Maximum Power of Y
      1 BCS-NPI digit (0-9).

  - id: cdpwrz
    type: str
    size: 1
    encoding: BCS-NPI
    doc: |
      Column Denominator Maximum Power of Z
      1 BCS-NPI digit (0-9).

  - id: cdtrms
    type: str
    size: 3
    encoding: BCS-NPI
    doc: |
      Number of Column Denominator Polynomial Terms
      3 BCS-NPI positive integer.

  - id: cdpcf
    type: str
    size: 21
    encoding: BCS-N
    repeat: expr
    repeat-expr: cdtrms.to_i
    doc: |
      Column Denominator Polynomial Coefficients
      CDTRMS coefficients, each 21 BCS-N real number.
