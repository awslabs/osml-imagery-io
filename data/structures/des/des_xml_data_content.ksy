meta:
  id: des_xml_data_content
  title: XML Data Content DES User-Defined Subheader
  endian: be

doc: |
  XML_DATA_CONTENT DES - XML Data Content Data Extension Segment
  
  This DES provides a mechanism for placing XML-formatted data content within
  NITF/NSIF files. The user-defined subheader fields (DESSHF) are conditional
  based on the DESSHL value:
  - 0000: No DESSHF fields
  - 0005: Only DESCRC field
  - 0283: DESCRC through DESSHTN fields
  - 0773: All fields (DESCRC through DESSHABS)
  
  Note: This definition covers the DES-specific subheader fields that appear
  in DESSHF when DESID is "XML_DATA_CONTENT". The DESDATA field contains
  the actual XML content.
  
  Reference: STDI-0002 Volume 2, Appendix F - XML_DATA_CONTENT

seq:
  - id: descrc
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Cyclic Redundancy Check (DESCRC)
      Calculated CRC value for the content of the DESDATA field.
      5 BCS-N positive integer (00000-65535 or 99999).
      A value of 99999 indicates CRC is not calculated.

  - id: desshft
    type: str
    size: 8
    encoding: BCS-A
    if: _root._io.size >= 13
    doc: |
      XML File Type (DESSHFT)
      Representative of the XML file type.
      8 BCS-A characters.
      Examples: XSD, XML, DTD, XSL, XSLT

  - id: desshdt
    type: str
    size: 20
    encoding: BCS-A
    if: _root._io.size >= 33
    doc: |
      Date and Time (DESSHDT)
      Time (UTC/Zulu) of the XML file's origination.
      20 BCS-A characters in format YYYY-MM-DDThh:mm:ssZ.

  - id: desshrp
    type: str
    size: 40
    encoding: UTF-8
    if: _root._io.size >= 73
    doc: |
      Responsible Party - Organization Identifier (DESSHRP)
      Identification of the organization responsible for the DES content.
      40 bytes UTF-8 encoded free text.

  - id: desshsi
    type: str
    size: 60
    encoding: UTF-8
    if: _root._io.size >= 133
    doc: |
      Specification Identifier (DESSHSI)
      Name of the specification used for the XML data content.
      60 bytes UTF-8 encoded free text.

  - id: desshsv
    type: str
    size: 10
    encoding: BCS-A
    if: _root._io.size >= 143
    doc: |
      Specification Version (DESSHSV)
      Version or edition of the specification.
      10 BCS-A characters free text.

  - id: desshsd
    type: str
    size: 20
    encoding: BCS-A
    if: _root._io.size >= 163
    doc: |
      Specification Date (DESSHSD)
      Version or edition date for the specification.
      20 BCS-A characters in format YYYY-MM-DDThh:mm:ssZ.

  - id: desshtn
    type: str
    size: 120
    encoding: BCS-A
    if: _root._io.size >= 283
    doc: |
      Target Namespace (DESSHTN)
      Identification of the target namespace designated within the XML content.
      120 BCS-A characters containing URL.
      Default is BCS spaces.

  - id: desshlpg
    type: str
    size: 125
    encoding: BCS-A
    if: _root._io.size >= 408
    doc: |
      Location - Polygon (DESSHLPG)
      Five-point boundary enclosing the area applicable to the DES.
      125 BCS-A characters containing five pairs of latitude/longitude values.
      Format: ±dd.dddddddd±ddd.dddddddd (repeated 5 times)
      Latitude range: -90 to +90, Longitude range: -180 to +360.
      Default is BCS spaces.

  - id: desshlpt
    type: str
    size: 25
    encoding: BCS-A
    if: _root._io.size >= 433
    doc: |
      Location - Point (DESSHLPT)
      Single geographic point applicable to the DES.
      25 BCS-A characters containing latitude/longitude pair.
      Format: ±dd.dddddddd±ddd.dddddddd
      Default is BCS spaces.

  - id: desshli
    type: str
    size: 20
    encoding: BCS-A
    if: _root._io.size >= 453
    doc: |
      Location - Identifier (DESSHLI)
      Identifier used to represent a geographic area.
      20 BCS-A characters.
      Examples: US, USA
      Default is BCS spaces.

  - id: desshlin
    type: str
    size: 120
    encoding: BCS-A
    if: _root._io.size >= 573
    doc: |
      Location Identifier Namespace URI (DESSHLIN)
      URI for the namespace where the Location Identifier is described.
      120 BCS-A characters.
      Default is BCS spaces.

  - id: desshabs
    type: str
    size: 200
    encoding: UTF-8
    if: _root._io.size >= 773
    doc: |
      Abstract (DESSHABS)
      Brief narrative summary of the content of the DES.
      200 bytes UTF-8 encoded free text.
      Default is BCS spaces.

