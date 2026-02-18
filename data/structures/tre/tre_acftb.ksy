meta:
  id: tre_acftb
  title: Aircraft Information TRE
  endian: be

doc: |
  ACFTB TRE - Aircraft Information Extension - Version B
  
  Provides miscellaneous information unique to airborne sensors.
  Required for all airborne imagery. A single ACFTB is placed in
  the respective subheader of every NITF image segment.
  
  Reference: STDI-0002 Volume 1, Appendix E - ASDE

seq:
  - id: ac_msn_id
    type: str
    size: 20
    encoding: ASCII
    doc: |
      Aircraft Mission Identification (AC_MSN_ID)
      20 BCS-A. "NOT AVAILABLE" if unavailable.

  - id: ac_tail_no
    type: str
    size: 10
    encoding: ASCII
    doc: |
      Aircraft Tail Number (AC_TAIL_NO)
      10 BCS-A.

  - id: ac_to
    type: str
    size: 12
    encoding: ASCII
    doc: |
      Aircraft Take-off (AC_TO)
      Date/time in UTC, CCYYMMDDhhmm format.
      12 BCS-A.

  - id: sensor_id_type
    type: str
    size: 4
    encoding: ASCII
    doc: |
      Sensor ID Type (SENSOR_ID_TYPE)
      Identifies sensor type (SAR, ccff for EO-IR, LIff for LiDAR).
      4 BCS-A.

  - id: sensor_id
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Sensor ID (SENSOR_ID)
      Identifies specific sensor that produced the image.
      6 BCS-A.

  - id: scene_source
    type: str
    size: 1
    encoding: ASCII
    doc: |
      Scene Source (SCENE_SOURCE)
      Origin of request for current scene.
      1 BCS-N, 0-9.

  - id: scnum
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Scene Number (SCNUM)
      Identifies current scene from mission plan.
      6 BCS-N, 000000-999999.

  - id: pdate
    type: str
    size: 8
    encoding: ASCII
    doc: |
      Processing Date (PDATE)
      Date raw data converted to imagery.
      8 BCS-A, CCYYMMDD format.

  - id: imhostno
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Immediate Scene Host (IMHOSTNO)
      Scene that immediate scene was initiated from.
      6 BCS-N, 000000-999999.

  - id: imreqid
    type: str
    size: 5
    encoding: ASCII
    doc: |
      Immediate Scene Request ID (IMREQID)
      Only non-zero for immediate scenes.
      5 BCS-N, 00000-99999.

  - id: mplan
    type: str
    size: 3
    encoding: ASCII
    doc: |
      Mission Plan Mode (MPLAN)
      Current sensor-specific collection mode.
      3 BCS-N, 001-999.

  - id: entloc
    type: str
    size: 25
    encoding: ASCII
    doc: |
      Entry Location (ENTLOC)
      Entry point latitude/longitude.
      25 BCS-A, ddmmss.ssssXdddmmss.ssssY or ±dd.dddddddd±ddd.dddddddd.

  - id: loc_accy
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Location Accuracy (LOC_ACCY)
      90% probable circular error in feet.
      6 BCS-N, 000.01-999.99 or 000000/000.00 for unknown.

  - id: entelv
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Entry Elevation (ENTELV)
      Entry point ground elevation.
      6 BCS-N, -01000 to +30000 feet or meters.

  - id: elv_unit
    type: str
    size: 1
    encoding: ASCII
    doc: |
      Unit of Elevation (ELV_UNIT)
      f=feet, m=meters.
      1 BCS-A.

  - id: exitloc
    type: str
    size: 25
    encoding: ASCII
    doc: |
      Exit Location (EXITLOC)
      Exit point latitude/longitude.
      25 BCS-A.

  - id: exitelv
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Exit Elevation (EXITELV)
      Exit point ground elevation.
      6 BCS-N.

  - id: tmap
    type: str
    size: 7
    encoding: ASCII
    doc: |
      True Map Angle (TMAP)
      Angle between ground projection and scene centerline.
      7 BCS-N, 000.000-180.000 degrees.

  - id: row_spacing
    type: str
    size: 7
    encoding: ASCII
    doc: |
      Row Spacing (ROW_SPACING)
      Distance between adjacent rows at image center.
      7 BCS-N.

  - id: row_spacing_units
    type: str
    size: 1
    encoding: ASCII
    doc: |
      Row Spacing Units (ROW_SPACING_UNITS)
      f=feet, m=meters, r=μ-radians, u=unknown.
      1 BCS-A.

  - id: col_spacing
    type: str
    size: 7
    encoding: ASCII
    doc: |
      Column Spacing (COL_SPACING)
      Distance between adjacent pixels within a row.
      7 BCS-N.

  - id: col_spacing_units
    type: str
    size: 1
    encoding: ASCII
    doc: |
      Column Spacing Units (COL_SPACING_UNITS)
      f=feet, m=meters, r=μ-radians, u=unknown.
      1 BCS-A.

  - id: focal_length
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Focal Length (FOCAL_LENGTH)
      Effective distance from optical lens to sensor.
      6 BCS-N, 000.01-899.99 cm, 999.99 if unavailable.

  - id: senserial
    type: str
    size: 6
    encoding: ASCII
    doc: |
      Sensor Serial Number (SENSERIAL)
      Vendor's serial number of sensor LRU.
      6 BCS-N, 000001-999999.

  - id: abswver
    type: str
    size: 7
    encoding: ASCII
    doc: |
      Airborne Software Version (ABSWVER)
      7 BCS-A.

  - id: cal_date
    type: str
    size: 8
    encoding: ASCII
    doc: |
      Calibration Date (CAL_DATE)
      Date sensor was last calibrated.
      8 BCS-A, CCYYMMDD format.

  - id: patch_tot
    type: str
    size: 4
    encoding: ASCII
    doc: |
      Patch Total (PATCH_TOT)
      Total number of patches in imaging operation.
      4 BCS-N, 0000-9999.

  - id: mti_tot
    type: str
    size: 3
    encoding: ASCII
    doc: |
      MTI Total (MTI_TOT)
      Total number of MTIRP extensions in file.
      3 BCS-N, 000-999.
