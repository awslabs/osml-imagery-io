meta:
  id: tre_aimidb
  title: Additional Image ID TRE
  endian: be

doc: |
  AIMIDB TRE - Additional Image ID Extension - Version B
  
  Used for storage and retrieval from standard imagery libraries.
  Required component of all airborne imagery files. A single AIMIDB
  is placed in the respective subheader of every NITF image segment.
  
  Reference: STDI-0002 Volume 1, Appendix E - ASDE

seq:
  - id: acquisition_date
    type: str
    size: 14
    encoding: ASCII
    doc: |
      Acquisition Date and Time (ACQUISITION_DATE)
      Date/time of collection in UTC.
      14 BCS-A, CCYYMMDDhhmmss format.

  - id: mission_no
    type: str
    size: 4
    encoding: ASCII
    doc: |
      Mission Number (MISSION_NO)
      Four character descriptor of the mission (PPNN format).
      4 BCS-A.

  - id: mission_identification
    type: str
    size: 10
    encoding: ASCII
    doc: |
      Mission Identification (MISSION_IDENTIFICATION)
      Name of the mission (Air Tasking Order Mission Number).
      10 BCS-A.

  - id: flight_no
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Flight Number (FLIGHT_NO)
      Flight number in range 01-09, A1-A9, etc.
      2 BCS-A.

  - id: op_num
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Image Operation Number (OP_NUM)
      Reset to 001 at start of each flight.
      3 BCS-N, 000-999.

  - id: current_segment
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Current Segment ID (CURRENT_SEGMENT)
      Identifies which segment of an imaging operation.
      2 BCS-A, AA-ZZ.

  - id: repro_num
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Reprocess Number (REPRO_NUM)
      Indicates if data was reprocessed.
      2 BCS-N, 00-99.

  - id: replay
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Replay (REPLAY)
      Indicates reprocessing or retransmission.
      3 BCS-A, 000, G01-G99, P01-P99, T01-T99.

  - id: reserved_001
    type: str
    size: 1
    encoding: ASCII
    doc: Reserved (1 space)

  - id: start_tile_column
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Starting Tile Column Number (START_TILE_COLUMN)
      For tiled sub-images, first tile column number.
      3 BCS-N, 001-099.

  - id: start_tile_row
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Starting Tile Row Number (START_TILE_ROW)
      For tiled sub-images, first tile row number.
      5 BCS-N, 00001-99999.

  - id: end_segment
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Ending Segment (END_SEGMENT)
      Ending segment ID of the imaging operation.
      2 BCS-A, 00 or AA-ZZ.

  - id: end_tile_column
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Ending Tile Column Number (END_TILE_COLUMN)
      For tiled sub-images, last tile column number.
      3 BCS-N, 001-099.

  - id: end_tile_row
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Ending Tile Row Number (END_TILE_ROW)
      For tiled sub-images, last tile row number.
      5 BCS-N, 00001-99999.

  - id: country
    type: str
    size: 2
    encoding: ASCII
    doc: |
      Country Code (COUNTRY)
      Two letter code for country of image reference point.
      2 BCS-A, AA-ZZ (FIPS PUB 10-4).

  - id: reserved_002
    type: str
    size: 4
    encoding: ASCII
    doc: Reserved (4 spaces)

  - id: location
    type: str
    size: 11
    encoding: ASCII
    doc: |
      Location (LOCATION)
      Natural reference point of sensor for geographic coverage.
      11 BCS-A, ddmmXdddmmY format.

  - id: reserved_003
    type: str
    size: 13
    encoding: ASCII
    doc: Reserved (13 spaces)
