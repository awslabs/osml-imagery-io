meta:
  id: tre_astora
  title: ASTOR Program Radar Data TRE
  endian: be

doc: |
  ASTORA TRE - ASTOR Program Radar Data Tagged Record Extension
  
  The ASTOR radar uses the Joint BIIF Profile standard to format its output data.
  This TRE provides radar-specific metadata for both Spot and Swath collection modes.
  
  The TRE has a fixed length of 711 bytes and contains:
  - Common fields for both Spot and Swath modes
  - Spot-specific fields (blank filled for Swath scenes)
  - Swath-specific fields (blank filled for Spot scenes)
  
  ASTOR uses the Multiple Correlated Files paradigm within NITF. The IMG_TOTAL_ROWS
  and IMG_TOTAL_COLS fields provide the size of the full image product that may have
  been divided over multiple NITF files.
  
  Reference: STDI-0002 Volume 1, Appendix AQ - ASTORA

seq:
  # Common Fields for Spot and Swath
  - id: IMG_TOTAL_ROWS
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Number of rows in full image product.
      6 BCS-N characters, range 000000-999999.

  - id: IMG_TOTAL_COLS
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Number of columns in full image product.
      6 BCS-N characters, range 000000-999999.

  - id: IMG_INDEX_ROW
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Upper left corner of tile, row coordinate, in full image coordinate grid.
      6 BCS-N characters, range 000000-999999.

  - id: IMG_INDEX_COL
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Upper left corner of tile, column coordinate, in full image coordinate grid.
      6 BCS-N characters, range 000000-999999.

  - id: GEOID_OFFSET
    type: str
    size: 7
    encoding: BCS-A
    doc: |
      Distance from the reference ellipsoid to the MSL geoid at the reference point (ft).
      7 BCS-A characters, format ±999.99.

  - id: ALPHA_0
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Cone Angle (radians).
      16 BCS-A characters, format ±9.9999999999999.

  - id: K_L
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Left/right look flag. 1 on left, -1 on right.
      2 BCS-A characters.

  - id: C_M
    type: str
    size: 15
    encoding: BCS-A
    doc: |
      Speed of light, adjusted for refractivity of atmosphere (m/s).
      15 BCS-A characters, format ddddddddd.ddddd.

  - id: AC_ROLL
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Nominal Aircraft Roll (radians).
      16 BCS-A characters, format ±9.9999999999999.

  - id: AC_PITCH
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Nominal Aircraft Pitch (radians).
      16 BCS-A characters, format ±9.9999999999999.

  - id: AC_YAW
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Nominal Aircraft Yaw (radians).
      16 BCS-A characters, format ±9.9999999999999.

  - id: AC_TRACK_HEADING
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Nominal Aircraft Track Heading (radians).
      16 BCS-A characters, format ±9.9999999999999.

  # Spot Fields (blank filled for Swath scenes)
  - id: AP_ORIGIN_X
    type: str
    size: 13
    encoding: BCS-A
    doc: |
      Synthetic aperture origin point, X component, in ECEF (m).
      Blank filled for Swath scenes.
      13 BCS-A characters, format ±99999999.999.

  - id: AP_ORIGIN_Y
    type: str
    size: 13
    encoding: BCS-A
    doc: |
      Synthetic aperture origin point, Y component, in ECEF (m).
      Blank filled for Swath scenes.
      13 BCS-A characters, format ±99999999.999.

  - id: AP_ORIGIN_Z
    type: str
    size: 13
    encoding: BCS-A
    doc: |
      Synthetic aperture origin point, Z component, in ECEF (m).
      Blank filled for Swath scenes.
      13 BCS-A characters, format ±99999999.999.

  - id: AP_DIR_X
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Synthetic aperture direction unit vector, X component, in ECEF.
      Blank filled for Swath scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: AP_DIR_Y
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Synthetic aperture direction unit vector, Y component, in ECEF.
      Blank filled for Swath scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: AP_DIR_Z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Synthetic aperture direction unit vector, Z component, in ECEF.
      Blank filled for Swath scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: X_AP_START
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Start point of synthetic aperture, relative to AP_ORIGIN along AP_DIR (m).
      Blank filled for Swath scenes.
      12 BCS-A characters, format ±99999.99999.

  - id: X_AP_END
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      End point of synthetic aperture, relative to AP_ORIGIN along AP_DIR (m).
      Blank filled for Swath scenes.
      12 BCS-A characters, format ±99999.99999.

  - id: SS_ROW_SHIFT
    type: str
    size: 4
    encoding: BCS-A
    doc: |
      Spot stitching row shift. Positive means the subspot was shifted down
      in the stitching function. Blank filled for Swath scenes.
      4 BCS-A characters, range -999 to 999.

  - id: SS_COL_SHIFT
    type: str
    size: 4
    encoding: BCS-A
    doc: |
      Spot stitching column shift. Positive means the subspot was shifted right
      in the stitching function. Blank filled for Swath scenes.
      4 BCS-A characters, range -999 to 999.

  # Swath Fields (blank filled for Spot scenes)
  - id: U_HAT_X
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The x component of u-hat (semi-major axis direction unit vector) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: U_HAT_Y
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The y component of u-hat (semi-major axis direction unit vector) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: U_HAT_Z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The z component of u-hat (semi-major axis direction unit vector) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: V_HAT_X
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The x component of v-hat (semi-minor axis direction unit vector) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: V_HAT_Y
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The y component of v-hat (semi-minor axis direction unit vector) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: V_HAT_Z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The z component of v-hat (semi-minor axis direction unit vector) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: N_HAT_X
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The x component of n-hat (normal vector to u and v) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: N_HAT_Y
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The y component of n-hat (normal vector to u and v) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: N_HAT_Z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The z component of n-hat (normal vector to u and v) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: ETA_0
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Dummy parameter for leading edge of patch (eta_0) (radians).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±9.9999999999999.

  - id: SIGMA_SM
    type: str
    size: 13
    encoding: BCS-A
    doc: |
      Semi-major axis length (m).
      Blank filled for Spot scenes.
      13 BCS-A characters, format ±99999999.999.

  - id: SIGMA_SN
    type: str
    size: 13
    encoding: BCS-A
    doc: |
      Semi-minor axis length (m).
      Blank filled for Spot scenes.
      13 BCS-A characters, format ±99999999.999.

  - id: S_OFF
    type: str
    size: 10
    encoding: BCS-A
    doc: |
      Small circle origin off-set along v-hat axis (m).
      Blank filled for Spot scenes.
      10 BCS-A characters, format ±9999.9999.

  - id: RN_OFFSET
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Off-set between small circle and great circle plane (m).
      Blank filled for Spot scenes.
      12 BCS-A characters, format ±999999.9999.

  - id: R_SCL
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Radius of curvature of scene center line (m).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ddddddd.dddddddd.

  - id: R_NAV
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Radius of curvature of nominal aircraft flight track (m).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ddddddd.dddddddd.

  - id: R_SC_EXACT
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Cross-track offset to scene center line (m).
      Blank filled for Spot scenes.
      16 BCS-A characters, format dddddd.ddddddddd.

  - id: C_SC_X
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      X component of center of scene center line fit circle in ECEF (m).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±999999.99999999.

  - id: C_SC_Y
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Y component of center of scene center line fit circle in ECEF (m).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±999999.99999999.

  - id: C_SC_Z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Z component of center of scene center line fit circle in ECEF (m).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±999999.99999999.

  - id: K_HAT_X
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      X component of local radial direction unit vector at leading edge of patch.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: K_HAT_Y
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Y component of local radial direction unit vector at leading edge of patch.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: K_HAT_Z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Z component of local radial direction unit vector at leading edge of patch.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: L_HAT_X
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      X component of local alongtrack direction unit vector at leading edge of patch.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: L_HAT_Y
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Y component of local alongtrack direction unit vector at leading edge of patch.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: L_HAT_Z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Z component of local alongtrack direction unit vector at leading edge of patch.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: P_Z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Perpendicular distance from navigation circle to scene center line circle (m).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±999999.99999999.

  - id: THETA_C
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Angular offset from target to aperture center (radians).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±9.9999999999999.

  - id: ALPHA_SL
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Tangent cone apex parameter.
      Blank filled for Spot scenes.
      16 BCS-A characters, format d.dddddddddddddd.

  - id: SIGMA_TC
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Tangent cone scale parameter.
      Blank filled for Spot scenes.
      16 BCS-A characters, format d.dddddddddddddd.
