meta:
  id: tre_stdidc
  title: Standard ID TRE
  endian: be

doc: |
  STDIDC TRE - Standard ID Extension
  
  Contains image identification data that supplements the image subheader.
  Used by USIGS compliant systems. A single STDIDC is placed in the image
  subheader; where several images relate to a single scene, an STDIDC may
  be placed in each applicable image subheader.
  
  Reference: STDI-0002 Volume 1, Appendix D - CSDE

seq:
  - id: acquisition_date
    type: str
    size: 14
    encoding: ASCII
    doc: |
      Acquisition Date (ACQUISITION_DATE)
      Date of collection mission (aircraft takeoff).
      14 BCS-A, YYYYMMDDHHMMSS format (UTC).

  - id: mission
    type: str
    size: 14
    encoding: ASCII
    doc: |
      Mission Identification (MISSION)
      Descriptor of the vehicle. For satellite, identifies specific vehicle.
      For aerial, identifies the scanner.
      14 BCS-A.

  - id: pass
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Pass Number (PASS)
      Identifies each pass or flight per day.
      2 BCS-A, 00-99, A1-A9, B1-B9, ... Z1-Z9.

  - id: op_num
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Image Operation Number (OP_NUM)
      Imaging operations numbers increase within each pass.
      3 BCS-N, 000-999. 000 indicates system doesn't number operations.

  - id: start_segment
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Start Segment ID (START_SEGMENT)
      Identifies images as separate pieces within an imaging operation.
      2 BCS-A, AA-ZZ. AA is first segment.

  - id: repro_num
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Reprocess Number (REPRO_NUM)
      Indicates if data was reprocessed or enhanced.
      2 BCS-N, 00-99. 00 is original, 01 is first reprocess.

  - id: replay_regen
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Replay/Regen (REPLAY_REGEN)
      Replay (remapping) or regeneration imagery mode.
      3 BCS-A. 000 indicates originally processed image.

  - id: blank_fill
    type: str
    size: 1
    encoding: ASCII
    doc: |
      Blank Fill (BLANK_FILL)
      1 BCS-A, space or underscore.

  - id: start_column
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Starting Column Block (START_COLUMN)
      Offset in blocks of first block in cross-scan direction.
      3 BCS-N, 001-999.

  - id: start_row
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Starting Row Block (START_ROW)
      Offset in blocks of first block in along-scan direction.
      5 BCS-N, 00001-99999.

  - id: end_segment
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Ending Segment ID (END_SEGMENT)
      Ending segment ID of this file.
      2 BCS-A, AA-ZZ.

  - id: end_column
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Ending Column Block (END_COLUMN)
      Offset in blocks of last block in cross-scan direction.
      3 BCS-N, 001-999.

  - id: end_row
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Ending Row Block (END_ROW)
      Offset in blocks of last block in along-scan direction.
      5 BCS-N, 00001-99999.

  - id: country
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Country Code (COUNTRY)
      Two letter code for country of image reference point.
      2 BCS-A, AA-ZZ (FIPS PUB 10-4).

  - id: wac
    type: str
    size: 4
    encoding: ASCII
    doc: |
      World Aeronautical Chart (WAC)
      4-number WAC for reference point of image segment.
      4 BCS-N, 0001-1866.

  - id: location
    type: str
    size: 11
    encoding: ASCII
    doc: |
      Location (LOCATION)
      Natural reference point of sensor for geographic coverage.
      11 BCS-A, DDMMXDDDMMY format.

  - id: reserved1
    type: str
    size: 5
    encoding: ASCII
    doc: Reserved (5 spaces)

  - id: reserved2
    type: str
    size: 8
    encoding: ASCII
    doc: Reserved (8 spaces)
