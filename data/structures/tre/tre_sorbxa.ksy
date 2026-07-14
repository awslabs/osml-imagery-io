meta:
  id: tre_sorbxa
  title: Space Object Orbit XML TRE
  endian: be

doc: |
  SORBXA TRE - Space Object Orbit XML Tagged Record Extension

  The Space Object Orbit XML, Version A (SORBXA) Tagged Record Extension (TRE)
  provides an explicit link between an image of a space object and its orbit geometry
  information. The SORBXA TRE is placed in the NITF image segment subheader with the
  orbit geometry encoded and referenced in the Common Sensor Ephemeris Data (CSEPHB)
  and optionally in the Common Sensor Attitude Data (CSATTB) data extension segments (DES),
  as defined in STDI-0002-2 App M.

  SORBXA TRE is intended primarily for Satellite-to-Satellite imaging of man-made objects
  traversing in the space environment and Non-Earth Imaging (NEI) which refers to the
  imaging of any object other than terrestrial or airborne objects (e.g. planes, drones,
  balloons).

  Reference: STDI-0002 Volume 1, Appendix AY - SORBXA

seq:
  - id: TREDATA
    type: str
    size-eos: true
    doc: |
      XML Data Content
      Contains a valid XML instance document conformant to the SORBXA XML Schema.
      The root element shall be "spaceObjectOrbitGeometry".
      The character set and encoding shall be declared within the XML encoding structure.
      Variable length.
