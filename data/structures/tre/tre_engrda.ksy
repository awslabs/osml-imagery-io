meta:
  id: tre_engrda
  title: Engineering Data TRE
  endian: be

doc: |
  ENGRDA TRE - Engineering Data Tagged Record Extension
  
  Provides a self-defining format for capturing and reporting engineering
  data that is not generally supported by other elements of the core NITF
  standard. Engineering data includes built-in-test (BIT) status, operational
  modes, status of system elements, and environmental conditions.
  
  The TRE uses a self-defining format where each data element includes
  metadata describing its type, size, units, and label. This allows
  downstream users to view the information without prior knowledge of
  the producing system.
  
  All numeric data is stored in big-endian format. Real data uses IEEE
  floating point format (ANSI/IEEE-754-1985).
  
  Reference: STDI-0002 Volume 1, Appendix N - ENGRDA

seq:
  - id: RESRC
    type: str
    size: 20
    encoding: BCS-A
    doc: |
      Unique Source System Name - Identifies the system that generated
      this engineering data. Preferably populated with the same System
      Identifier as used in the ACFT SDE "SENSOR_ID" field.
      Left justified, space padded.

  - id: RECNT
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Record Entry Count - Number of Engineering Data Elements included
      in the TRE. Range: 001 to 999.

  - id: RECORDS
    type: engineering_record
    repeat: expr
    repeat-expr: RECNT.to_i
    doc: Engineering data records.

types:
  engineering_record:
    doc: |
      A single engineering data element containing a label, matrix dimensions,
      data type, size, units, and the actual data values.
    seq:
      - id: ENGLN
        type: str
        size: 2
        encoding: BCS-N
        doc: |
          Engineering Data Label Length - Length in bytes of the label.
          Range: 01 to 99.

      - id: ENGLBL
        type: str
        size: ENGLN.to_i
        encoding: BCS-A
        doc: |
          Engineering Data Label - Unique string identifying the engineering
          data. No terminator characters (0x0A or 0x0D) allowed.

      - id: ENGMTXC
        type: str
        size: 4
        encoding: BCS-N
        doc: |
          Engineering Matrix Data Column Count - Number of elements in each
          row of the matrix data (C). For one-dimensional arrays, this is
          the number of elements. Range: 0001 to 9999.

      - id: ENGMTXR
        type: str
        size: 4
        encoding: BCS-N
        doc: |
          Engineering Matrix Data Row Count - Number of rows in the matrix
          data (R). Matrix elements are stored as a vector in C x R order.
          Range: 0001 to 9999.

      - id: ENGTYP
        type: str
        size: 1
        encoding: BCS-A
        doc: |
          Value Type of Engineering Data Element:
          B = Binary data
          I = Unsigned Integer data
          S = Signed Integer data
          R = Real Number (IEEE floating point)
          C = Complex data (pair of real elements)
          A = BCS Alphanumeric character data

      - id: ENGDTS
        type: str
        size: 1
        encoding: BCS-N
        doc: |
          Engineering Data Element Size - Number of bytes per data element.
          Expected values: 1, 2, 4, 8, etc.
          For BCS data (ENGTYP=A), this shall be "1".
          For IEEE float (ENGTYP=R), minimum is 4 (32 bits).

      - id: ENGDATU
        type: str
        size: 2
        encoding: BCS-A
        doc: |
          Engineering Data Units - Units for the data values.
          Examples: ft (feet), m (meters), in (inch), mm (millimeters),
          mi/nm (miles), km (kilometer), kt (knots), NA (not applicable),
          tC/tF/tK (temperature), Va/Vd (voltage), mA/uA/A (current),
          UD (undefined). ISO-1000 should guide other unit definitions.

      - id: ENGDATC
        type: str
        size: 8
        encoding: BCS-N
        doc: |
          Engineering Data Count - Number of data symbols in this record.
          For BCS data, this is the byte/character count.
          Range: 00000001 to 99999932.

      - id: ENGDATA
        size: ENGDATC.to_i
        doc: |
          Engineering Data - The actual engineering data values.
          Format depends on ENGTYP:
          - Binary/Integer/Signed: raw bytes in big-endian order
          - Real: IEEE 754 floating point
          - Complex: pairs of IEEE floats
          - Alphanumeric: BCS characters
