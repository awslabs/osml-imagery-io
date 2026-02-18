meta:
  id: tre_xmldca
  title: XML Data Content TRE
  endian: be

doc: |
  XMLDCA TRE - XML Data Content Tagged Record Extension
  
  Provides a mechanism for placing XML-formatted data content within the
  file header and segment sub-header extension areas of NITF files.
  
  The TRE contains:
  - User-defined subheader with metadata about the XML content
  - The actual XML data in the TREDATA field
  
  The subheader fields are conditional based on TRESHL value:
  - 0000: No subheader fields
  - 0005: Only TRECRC
  - 0283: TRECRC through TRESHTN
  - 0773: All subheader fields
  
  Reference: STDI-0002 Volume 1, Appendix AE - XMLDCA

seq:
  - id: treshl
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      User-defined Subheader Length (TRESHL)
      Number of bytes in the TRESHF field.
      0000 = No subheader, 0005 = TRECRC only,
      0283 = TRECRC through TRESHTN, 0773 = All fields.
      4 BCS-N characters.

  - id: trecrc
    type: str
    size: 5
    encoding: BCS-N
    if: treshl.to_i >= 5
    doc: |
      Cyclic Redundancy Check (TRECRC)
      CRC-16 value for the TREDATA field content.
      Value 99999 indicates CRC not calculated.
      5 BCS-N characters, range 00000-65535 or 99999.

  - id: treshft
    type: str
    size: 8
    encoding: BCS-A
    if: treshl.to_i >= 283
    doc: |
      XML File Type (TRESHFT)
      Representative of the XML file type.
      Examples: XSD, XML, DTD, XSL, XSLT.
      8 BCS-A characters.

  - id: treshdt
    type: str
    size: 20
    encoding: BCS-A
    if: treshl.to_i >= 283
    doc: |
      Date and Time (TRESHDT)
      UTC time of the XML file's origination.
      Format: YYYY-MM-DDThh:mm:ssZ.
      20 BCS-A characters.

  - id: treshrp
    type: str
    size: 40
    encoding: BCS-A
    if: treshl.to_i >= 283
    doc: |
      Responsible Party - Organization Identifier (TRESHRP)
      Identification of the organization responsible for the TRE content.
      40 BCS-A characters, free text.

  - id: treshsi
    type: str
    size: 60
    encoding: BCS-A
    if: treshl.to_i >= 283
    doc: |
      Specification Identifier (TRESHSI)
      Name of the specification used for the XML data content.
      60 BCS-A characters, free text.

  - id: treshsv
    type: str
    size: 10
    encoding: BCS-A
    if: treshl.to_i >= 283
    doc: |
      Specification Version (TRESHSV)
      Version or edition of the specification.
      10 BCS-A characters, free text.

  - id: treshsd
    type: str
    size: 20
    encoding: BCS-A
    if: treshl.to_i >= 283
    doc: |
      Specification Date (TRESHSD)
      Version or edition date for the specification.
      Format: YYYY-MM-DDThh:mm:ssZ.
      20 BCS-A characters.

  - id: treshtn
    type: str
    size: 120
    encoding: BCS-A
    if: treshl.to_i >= 283
    doc: |
      Target Namespace (TRESHTN)
      Identification of the target namespace designated within the XML data.
      Default is spaces.
      120 BCS-A characters, URL.

  - id: treshlpg
    type: str
    size: 125
    encoding: BCS-A
    if: treshl.to_i >= 773
    doc: |
      Location - Polygon (TRESHLPG)
      Five-point boundary enclosing the area applicable to the TRE.
      Five pairs of latitude/longitude values in decimal degrees.
      Format: ±dd.dddddddd±ddd.dddddddd (repeated 5 times).
      Default is spaces.
      125 BCS-A characters.

  - id: treshlpt
    type: str
    size: 25
    encoding: BCS-A
    if: treshl.to_i >= 773
    doc: |
      Location - Point (TRESHLPT)
      Single geographic point applicable to the TRE.
      Format: ±dd.dddddddd±ddd.dddddddd.
      Default is spaces.
      25 BCS-A characters.

  - id: treshli
    type: str
    size: 20
    encoding: BCS-A
    if: treshl.to_i >= 773
    doc: |
      Location - Identifier (TRESHLI)
      Identifier used to represent a geographic area.
      Examples: US, USA.
      Default is spaces.
      20 BCS-A characters.

  - id: treshlin
    type: str
    size: 120
    encoding: BCS-A
    if: treshl.to_i >= 773
    doc: |
      Location Identifier Namespace URI (TRESHLIN)
      URI for the namespace where the Location Identifier is described.
      Default is spaces.
      120 BCS-A characters.

  - id: treshabs
    type: str
    size: 200
    encoding: BCS-A
    if: treshl.to_i >= 773
    doc: |
      Abstract (TRESHABS)
      Brief narrative summary of the content of the TRE.
      Default is spaces.
      200 BCS-A characters, free text.

  - id: tredata
    size-eos: true
    doc: |
      User-defined Data Field (TREDATA)
      Contains the XML data. The character set and encoding shall be
      declared within the XML encoding structure.
      Variable length.
