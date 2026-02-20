meta:
  id: tre_soddxa
  title: Space Object Description Data XML TRE
  endian: be

doc: |
  SODDXA TRE - Space Object Description Data XML Tagged Record Extension
  
  Describes the key characteristics of an imaged space object in an XML format.
  The TRE is designed for Non-Earth Imaging (NEI) from any domain (ground, sea,
  air, space) by providing information about the imaged space object.
  
  The CEDATA field contains a valid XML instance document conformant to the
  SODDXA XML Schema Definition. The root element shall be "spaceObjectDescriptionData".
  
  The XML content includes information about:
  - Space object identification (satellite number, catalog source)
  - Space object type and category
  - Orbital parameters
  - Physical characteristics
  - Operational status
  - Associated organizations
  
  Reference: STDI-0002 Volume 1, Appendix AP - SODDXA v1.0

seq:
  - id: XML_DATA
    size-eos: true
    doc: |
      XML Data Content
      Contains a valid XML instance document conformant to the SODDXA XML Schema.
      The root element shall be "spaceObjectDescriptionData".
      The character set and encoding shall be declared within the XML encoding structure.
      Variable length.
