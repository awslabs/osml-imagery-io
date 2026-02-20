meta:
  id: tre_piaimc
  title: Profile for Imagery Access Image TRE
  endian: be

doc: |
  PIAIMC TRE - Profile for Imagery Access Image - Version C
  
  Provides fields not currently carried in NITF but contained in the
  Standards Profile for Imagery Access (SPIA). Contains imagery-related
  information including cloud cover, sensor mode, and processing details.
  
  Reference: STDI-0002 Volume 1, Appendix C - PIAE

seq:
  - id: CLOUDCVR
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Cloud Cover (CLOUDCVR)
      Percentage of image obscured by cloud.
      3 BCS-N, 000-100 or 999 (unknown).

  - id: SRP
    type: str
    size: 1
    encoding: ASCII
    doc: |
      Standard Radiometric Product (SRP)
      Indicates if standard radiometric product data is available.
      1 BCS-A, Y or N.

  - id: SENSMODE
    type: str
    size: 12
    encoding: ASCII
    doc: |
      Sensor Mode (SENSMODE)
      Identifies the sensor mode used in capturing the image.
      12 BCS-A, e.g., WHISKBROOM, PUSHBROOM, FRAMING, SPOT, SWATH.

  - id: SENSNAME
    type: str
    size: 18
    encoding: ASCII
    doc: |
      Sensor Name (SENSNAME)
      Identifies the name of the sensor used in capturing the image.
      18 BCS-A.

  - id: SOURCE
    type: str
    size: 255
    encoding: ASCII
    doc: |
      Source (SOURCE)
      Indicates where the image came from.
      255 BCS-A.

  - id: COMGEN
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Compression Generation (COMGEN)
      Number of lossy compressions done by the archive.
      2 BCS-N, 00-99.

  - id: SUBQUAL
    type: str
    size: 1
    encoding: ASCII
    doc: |
      Subjective Quality (SUBQUAL)
      Subjective rating of image quality.
      1 BCS-A, P=Poor, G=Good, E=Excellent, F=Fair.

  - id: PIAMSNNUM
    type: str
    size: 7
    encoding: ASCII
    doc: |
      PIA Mission Number (PIAMSNNUM)
      Mission number assigned to the reconnaissance mission.
      7 BCS-A.

  - id: CAMSPECS
    type: str
    size: 32
    encoding: ASCII
    doc: |
      Camera Specs (CAMSPECS)
      Brand name of camera and focal length of lens.
      32 BCS-A.

  - id: PROJID
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Project ID Code (PROJID)
      Collection platform project identifier code.
      2 BCS-A.

  - id: GENERATION
    type: str
    size: 1
    encoding: ASCII
    doc: |
      Generation (GENERATION)
      Number of image generations. 0 is reserved for original.
      1 BCS-N, 0-9.

  - id: ESD
    type: str
    size: 1
    encoding: ASCII
    doc: |
      Exploitation Support Data (ESD)
      Indicates if exploitation support data is available.
      1 BCS-A, Y or N.

  - id: OTHERCOND
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Other Conditions (OTHERCOND)
      Other conditions affecting imagery over target.
      2 BCS-A.

  - id: MEANGSD
    type: str
    size: 7
    encoding: ASCII
    doc: |
      Mean GSD (MEANGSD)
      Geometric mean of across/along scan center-to-center distance.
      7 BCS-N, 00000.0 to 99999.9 in inches.

  - id: IDATUM
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Image Datum (IDATUM)
      Mathematical representation of earth used to geo-correct image.
      3 BCS-A.

  - id: IELLIP
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Image Ellipsoid (IELLIP)
      Mathematical representation of earth (ellipsoid) for geo-correction.
      3 BCS-A.

  - id: PREPROC
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Image Processing Level Code (PREPROC)
      Level of radiometric and geometric processing applied.
      2 BCS-A.

  - id: IPROJ
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Image Projection System (IPROJ)
      2D-map projection used to geo-correct/rectify image.
      2 BCS-A.

  - id: SATTRACK
    type: str
    size: 8
    encoding: ASCII
    doc: |
      Satellite Track (SATTRACK)
      Location based on system's Earth grid (PATH/ROW).
      8 BCS-N, PPPPRRRR format.
