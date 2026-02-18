meta:
  id: tre_maplob
  title: Map Projected Location TRE
  endian: be

doc: |
  MAPLOB TRE - Map Projected Location Tagged Record Extension
  
  Provides map projected location information for NITF images.
  Contains scale factors and origin coordinates in projected
  coordinate system units.
  
  Reference: STDI-0002 Volume 1, Appendix P - GEOSDE

seq:
  - id: uni
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Unit of Measure (UNI)
      3 BCS-A. Unit for coordinate values.
      Values: "M  " (Meters), "DM " (Decimeters),
      "CM " (Centimeters), "MM " (Millimeters),
      "UM " (Micrometers), "KM " (Kilometers),
      "F  " (Feet), "I  " (Inches).

  - id: arv
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      X Scale Factor (ARV)
      9 BCS-N positive integer. Number of pixels per unit
      in the X (easting) direction.

  - id: brv
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Y Scale Factor (BRV)
      9 BCS-N positive integer. Number of pixels per unit
      in the Y (northing) direction.

  - id: lso
    type: str
    size: 15
    encoding: BCS-N
    doc: |
      X Origin (LSO)
      15 BCS-N real number. X coordinate (easting) of the
      origin of the image coordinate system.

  - id: pso
    type: str
    size: 15
    encoding: BCS-N
    doc: |
      Y Origin (PSO)
      15 BCS-N real number. Y coordinate (northing) of the
      origin of the image coordinate system.
