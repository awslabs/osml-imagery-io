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
  - id: UNILOA
    type: str
    size: 3
    encoding: BCS-A
    doc: |
      Length Units (UNILOA)
      3 BCS-A. Unit of measure used for the Easting (LOD) and
      Northing (LAD) intervals. Default value is "M" (meters).
      Values: "UM" (micrometers), "MM" (millimeters),
      "CM" (centimeters), "DM" (decimeters), "M" (meters),
      "KM" (kilometers), "IN" (inches), "FT" (feet),
      "YD" (yards), "MI" (statute miles), "NM" (nautical miles),
      or all BCS spaces.

  - id: LOD
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Easting Interval (LOD)
      5 BCS-N. Data density in the E/W direction that is the
      column width of an image pixel (00001 to 99999).

  - id: LAD
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Northing Interval (LAD)
      5 BCS-N. Data density in the N/S direction that is the
      line width of an image pixel (00001 to 99999).

  - id: LSO
    type: str
    size: 15
    encoding: BCS-N
    doc: |
      X Origin (LSO)
      15 BCS-N real number. X coordinate (easting) of the
      origin of the image coordinate system.

  - id: PSO
    type: str
    size: 15
    encoding: BCS-N
    doc: |
      Y Origin (PSO)
      15 BCS-N real number. Y coordinate (northing) of the
      origin of the image coordinate system.
