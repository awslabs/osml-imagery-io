meta:
  id: tre_geolob
  title: Geographic Location TRE
  endian: be

doc: |
  GEOLOB TRE - Geographic Location Tagged Record Extension
  
  Provides geographic location information for NITF images using
  a simple geographic coordinate system. Contains longitude and
  latitude density values along with reference origin coordinates.
  
  Reference: STDI-0002 Volume 1, Appendix P - GEOSDE

seq:
  - id: ARV
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Longitude Density (ARV)
      Number of pixels or elements per 360 degrees of longitude.
      9 BCS-N positive integer.

  - id: BRV
    type: str
    size: 9
    encoding: BCS-N
    doc: |
      Latitude Density (BRV)
      Number of pixels or elements per 360 degrees of latitude.
      9 BCS-N positive integer.

  - id: LSO
    type: str
    size: 15
    encoding: BCS-N
    doc: |
      Longitude of Reference Origin (LSO)
      Longitude of the origin of the coordinate system in degrees.
      15 BCS-N real number, range ±180.000000000.

  - id: PSO
    type: str
    size: 15
    encoding: BCS-N
    doc: |
      Latitude of Reference Origin (PSO)
      Latitude of the origin of the coordinate system in degrees.
      15 BCS-N real number, range ±90.0000000000.
