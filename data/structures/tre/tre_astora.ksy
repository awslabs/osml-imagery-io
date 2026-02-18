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
  - id: img_total_rows
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Number of rows in full image product.
      6 BCS-N characters, range 000000-999999.

  - id: img_total_cols
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Number of columns in full image product.
      6 BCS-N characters, range 000000-999999.

  - id: img_index_row
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Upper left corner of tile, row coordinate, in full image coordinate grid.
      6 BCS-N characters, range 000000-999999.

  - id: img_index_col
    type: str
    size: 6
    encoding: BCS-N
    doc: |
      Upper left corner of tile, column coordinate, in full image coordinate grid.
      6 BCS-N characters, range 000000-999999.

  - id: geoid_offset
    type: str
    size: 7
    encoding: BCS-A
    doc: |
      Distance from the reference ellipsoid to the MSL geoid at the reference point (ft).
      7 BCS-A characters, format ±999.99.

  - id: alpha_0
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Cone Angle (radians).
      16 BCS-A characters, format ±9.9999999999999.

  - id: k_l
    type: str
    size: 2
    encoding: BCS-A
    doc: |
      Left/right look flag. 1 on left, -1 on right.
      2 BCS-A characters.

  - id: c_m
    type: str
    size: 15
    encoding: BCS-A
    doc: |
      Speed of light, adjusted for refractivity of atmosphere (m/s).
      15 BCS-A characters, format ddddddddd.ddddd.

  - id: ac_roll
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Nominal Aircraft Roll (radians).
      16 BCS-A characters, format ±9.9999999999999.

  - id: ac_pitch
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Nominal Aircraft Pitch (radians).
      16 BCS-A characters, format ±9.9999999999999.

  - id: ac_yaw
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Nominal Aircraft Yaw (radians).
      16 BCS-A characters, format ±9.9999999999999.

  - id: ac_track_heading
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Nominal Aircraft Track Heading (radians).
      16 BCS-A characters, format ±9.9999999999999.

  # Spot Fields (blank filled for Swath scenes)
  - id: ap_origin_x
    type: str
    size: 13
    encoding: BCS-A
    doc: |
      Synthetic aperture origin point, X component, in ECEF (m).
      Blank filled for Swath scenes.
      13 BCS-A characters, format ±99999999.999.

  - id: ap_origin_y
    type: str
    size: 13
    encoding: BCS-A
    doc: |
      Synthetic aperture origin point, Y component, in ECEF (m).
      Blank filled for Swath scenes.
      13 BCS-A characters, format ±99999999.999.

  - id: ap_origin_z
    type: str
    size: 13
    encoding: BCS-A
    doc: |
      Synthetic aperture origin point, Z component, in ECEF (m).
      Blank filled for Swath scenes.
      13 BCS-A characters, format ±99999999.999.

  - id: ap_dir_x
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Synthetic aperture direction unit vector, X component, in ECEF.
      Blank filled for Swath scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: ap_dir_y
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Synthetic aperture direction unit vector, Y component, in ECEF.
      Blank filled for Swath scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: ap_dir_z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Synthetic aperture direction unit vector, Z component, in ECEF.
      Blank filled for Swath scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: x_ap_start
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Start point of synthetic aperture, relative to AP_ORIGIN along AP_DIR (m).
      Blank filled for Swath scenes.
      12 BCS-A characters, format ±99999.99999.

  - id: x_ap_end
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      End point of synthetic aperture, relative to AP_ORIGIN along AP_DIR (m).
      Blank filled for Swath scenes.
      12 BCS-A characters, format ±99999.99999.

  - id: ss_row_shift
    type: str
    size: 4
    encoding: BCS-A
    doc: |
      Spot stitching row shift. Positive means the subspot was shifted down
      in the stitching function. Blank filled for Swath scenes.
      4 BCS-A characters, range -999 to 999.

  - id: ss_col_shift
    type: str
    size: 4
    encoding: BCS-A
    doc: |
      Spot stitching column shift. Positive means the subspot was shifted right
      in the stitching function. Blank filled for Swath scenes.
      4 BCS-A characters, range -999 to 999.

  # Swath Fields (blank filled for Spot scenes)
  - id: u_hat_x
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The x component of u-hat (semi-major axis direction unit vector) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: u_hat_y
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The y component of u-hat (semi-major axis direction unit vector) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: u_hat_z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The z component of u-hat (semi-major axis direction unit vector) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: v_hat_x
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The x component of v-hat (semi-minor axis direction unit vector) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: v_hat_y
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The y component of v-hat (semi-minor axis direction unit vector) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: v_hat_z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The z component of v-hat (semi-minor axis direction unit vector) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: n_hat_x
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The x component of n-hat (normal vector to u and v) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: n_hat_y
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The y component of n-hat (normal vector to u and v) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: n_hat_z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      The z component of n-hat (normal vector to u and v) in ECEF.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: eta_0
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Dummy parameter for leading edge of patch (eta_0) (radians).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±9.9999999999999.

  - id: sigma_sm
    type: str
    size: 13
    encoding: BCS-A
    doc: |
      Semi-major axis length (m).
      Blank filled for Spot scenes.
      13 BCS-A characters, format ±99999999.999.

  - id: sigma_sn
    type: str
    size: 13
    encoding: BCS-A
    doc: |
      Semi-minor axis length (m).
      Blank filled for Spot scenes.
      13 BCS-A characters, format ±99999999.999.

  - id: s_off
    type: str
    size: 10
    encoding: BCS-A
    doc: |
      Small circle origin off-set along v-hat axis (m).
      Blank filled for Spot scenes.
      10 BCS-A characters, format ±9999.9999.

  - id: rn_offset
    type: str
    size: 12
    encoding: BCS-A
    doc: |
      Off-set between small circle and great circle plane (m).
      Blank filled for Spot scenes.
      12 BCS-A characters, format ±999999.9999.

  - id: r_scl
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Radius of curvature of scene center line (m).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ddddddd.dddddddd.

  - id: r_nav
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Radius of curvature of nominal aircraft flight track (m).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ddddddd.dddddddd.

  - id: r_sc_exact
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Cross-track offset to scene center line (m).
      Blank filled for Spot scenes.
      16 BCS-A characters, format dddddd.ddddddddd.

  - id: c_sc_x
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      X component of center of scene center line fit circle in ECEF (m).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±999999.99999999.

  - id: c_sc_y
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Y component of center of scene center line fit circle in ECEF (m).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±999999.99999999.

  - id: c_sc_z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Z component of center of scene center line fit circle in ECEF (m).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±999999.99999999.

  - id: k_hat_x
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      X component of local radial direction unit vector at leading edge of patch.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: k_hat_y
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Y component of local radial direction unit vector at leading edge of patch.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: k_hat_z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Z component of local radial direction unit vector at leading edge of patch.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: l_hat_x
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      X component of local alongtrack direction unit vector at leading edge of patch.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: l_hat_y
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Y component of local alongtrack direction unit vector at leading edge of patch.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: l_hat_z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Z component of local alongtrack direction unit vector at leading edge of patch.
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±1.0000000000000.

  - id: p_z
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Perpendicular distance from navigation circle to scene center line circle (m).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±999999.99999999.

  - id: theta_c
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Angular offset from target to aperture center (radians).
      Blank filled for Spot scenes.
      16 BCS-A characters, format ±9.9999999999999.

  - id: alpha_sl
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Tangent cone apex parameter.
      Blank filled for Spot scenes.
      16 BCS-A characters, format d.dddddddddddddd.

  - id: sigma_tc
    type: str
    size: 16
    encoding: BCS-A
    doc: |
      Tangent cone scale parameter.
      Blank filled for Spot scenes.
      16 BCS-A characters, format d.dddddddddddddd.
