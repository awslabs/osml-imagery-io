//! GeoTIFF metadata parsing and building.
//!
//! Pure-Rust implementation of GeoKey directory parsing (tag 34735), double/ASCII
//! parameter resolution, transformation tag parsing, and the inverse operations
//! for writing GeoTIFF metadata. No libgeotiff dependency.

use std::collections::HashMap;

use serde_json::Value;

use crate::error::CodecError;

#[cfg(test)]
use crate::tiff::tags;

/// Convert an f64 to a serde_json::Value, using integer representation when possible.
#[cfg(test)]
fn json_f64(v: f64) -> Value {
    if v.fract() == 0.0 && v.is_finite() && v.abs() < (i64::MAX as f64) {
        Value::from(v as i64)
    } else {
        Value::from(v)
    }
}

/// Parse the GeoKey directory and resolve all key values to metadata fields.
///
/// # Arguments
/// - `directory` — raw u16 array from tag 34735
/// - `double_params` — optional f64 array from tag 34736
/// - `ascii_params` — optional string from tag 34737
///
/// # Returns
/// A HashMap of `"Geo"`-prefixed metadata fields.
/// Parse the GeoKey directory and resolve all key values to metadata fields.
///
/// # Deprecated
///
/// This function is no longer used in the read path. The TIFF metadata provider
/// now stores GeoTIFF tags (34735, 34736, 34737) under their numeric keys and
/// does not decode GeoKey directory contents. This function is retained only for
/// use in write-path roundtrip tests. Prefer reading raw numeric tag values
/// directly from the Tag_Dictionary.
///
/// # Arguments
/// - `directory` — raw u16 array from tag 34735
/// - `double_params` — optional f64 array from tag 34736
/// - `ascii_params` — optional string from tag 34737
///
/// # Returns
/// A HashMap of `"Geo"`-prefixed metadata fields.
#[cfg(test)]
fn parse_geokeys(
    directory: &[u16],
    double_params: Option<&[f64]>,
    ascii_params: Option<&str>,
) -> Result<HashMap<String, Value>, CodecError> {
    if directory.len() < 4 {
        return Err(CodecError::Decode(format!(
            "GeoKey directory too short: expected at least 4 values (header), got {}",
            directory.len()
        )));
    }

    let num_keys = directory[3] as usize;
    let mut result = HashMap::new();

    for i in 0..num_keys {
        let base = 4 + i * 4;
        if base + 3 >= directory.len() {
            break;
        }
        let key_id = directory[base];
        let tiff_tag_location = directory[base + 1];
        let count = directory[base + 2];
        let value_offset = directory[base + 3];

        let value = match tiff_tag_location {
            0 => {
                // Inline SHORT value
                resolve_inline_key(key_id, value_offset)
            }
            loc if loc == tags::GEO_DOUBLE_PARAMS_TAG as u16 => {
                let doubles = double_params.ok_or_else(|| {
                    CodecError::Decode(format!(
                        "GeoKey {} references GeoDoubleParamsTag (34736) but tag is not present",
                        key_id
                    ))
                })?;
                let off = value_offset as usize;
                let cnt = count as usize;
                if off + cnt > doubles.len() {
                    return Err(CodecError::Decode(format!(
                        "GeoKey {} references double params at offset {} count {} but array has {} elements",
                        key_id, off, cnt, doubles.len()
                    )));
                }
                if cnt == 1 {
                    resolve_double_key(key_id, doubles[off])
                } else {
                    // Multiple doubles — use generic key name with array
                    let arr: Vec<Value> = doubles[off..off + cnt]
                        .iter()
                        .map(|&d| json_f64(d))
                        .collect();
                    (format!("GeoKey_{}", key_id), Value::Array(arr))
                }
            }
            loc if loc == tags::GEO_ASCII_PARAMS_TAG as u16 => {
                let ascii = ascii_params.ok_or_else(|| {
                    CodecError::Decode(format!(
                        "GeoKey {} references GeoAsciiParamsTag (34737) but tag is not present",
                        key_id
                    ))
                })?;
                let off = value_offset as usize;
                let cnt = count as usize;
                if off + cnt > ascii.len() {
                    return Err(CodecError::Decode(format!(
                        "GeoKey {} references ASCII params at offset {} count {} but string has {} chars",
                        key_id, off, cnt, ascii.len()
                    )));
                }
                let s = ascii[off..off + cnt].trim_end_matches('|');
                resolve_ascii_key(key_id, s)
            }
            _ => {
                // Unknown TIFFTagLocation — treat as inline SHORT
                resolve_inline_key(key_id, value_offset)
            }
        };

        result.insert(value.0, value.1);
    }

    Ok(result)
}

/// Resolve an inline SHORT GeoKey value to a (key_name, Value) pair.
/// Resolve an inline SHORT GeoKey value to a (key_name, Value) pair.
#[cfg(test)]
fn resolve_inline_key(key_id: u16, value: u16) -> (String, Value) {
    match key_id {
        tags::GT_MODEL_TYPE_GEO_KEY => {
            let label = match value {
                tags::MODEL_TYPE_PROJECTED => "Projected",
                tags::MODEL_TYPE_GEOGRAPHIC => "Geographic",
                _ => return (format!("GeoKey_{}", key_id), Value::from(value)),
            };
            ("GeoModelType".to_string(), Value::String(label.to_string()))
        }
        tags::GT_RASTER_TYPE_GEO_KEY => {
            let label = match value {
                tags::RASTER_PIXEL_IS_AREA => "PixelIsArea",
                tags::RASTER_PIXEL_IS_POINT => "PixelIsPoint",
                _ => return (format!("GeoKey_{}", key_id), Value::from(value)),
            };
            (
                "GeoRasterType".to_string(),
                Value::String(label.to_string()),
            )
        }
        tags::PROJECTED_CS_TYPE_GEO_KEY => ("GeoProjectedCRS".to_string(), Value::from(value)),
        tags::GEOGRAPHIC_TYPE_GEO_KEY => ("GeoGeographicCRS".to_string(), Value::from(value)),
        _ => (format!("GeoKey_{}", key_id), Value::from(value)),
    }
}

/// Resolve a DOUBLE-referenced GeoKey value.
/// Resolve a DOUBLE-referenced GeoKey value.
#[cfg(test)]
fn resolve_double_key(key_id: u16, value: f64) -> (String, Value) {
    match key_id {
        tags::PROJECTED_CS_TYPE_GEO_KEY => ("GeoProjectedCRS".to_string(), json_f64(value)),
        tags::GEOGRAPHIC_TYPE_GEO_KEY => ("GeoGeographicCRS".to_string(), json_f64(value)),
        _ => (format!("GeoKey_{}", key_id), json_f64(value)),
    }
}

/// Resolve an ASCII-referenced GeoKey value.
/// Resolve an ASCII-referenced GeoKey value.
#[cfg(test)]
fn resolve_ascii_key(key_id: u16, value: &str) -> (String, Value) {
    (
        format!("GeoKey_{}", key_id),
        Value::String(value.to_string()),
    )
}

/// Build a GeoKey directory and associated parameter arrays from numeric metadata keys.
///
/// Reads the raw GeoKey directory from `"34735"`, optional double params from `"34736"`,
/// and optional ASCII params from `"34737"` in the Tag_Dictionary.
///
/// Returns `(directory_u16_array, Option<double_params>, Option<ascii_params>)`.
/// Returns an empty directory vec if `"34735"` is not present.
pub fn build_geokey_directory(
    metadata: &HashMap<String, Value>,
) -> Result<(Vec<u16>, Option<Vec<f64>>, Option<String>), CodecError> {
    // Read raw GeoKey directory from numeric key "34735"
    let directory = match metadata.get("34735") {
        Some(val) => {
            let arr = val.as_array().ok_or_else(|| {
                CodecError::Encode(
                    "GeoKeyDirectoryTag (34735) must be a JSON array of integers".into(),
                )
            })?;
            arr.iter()
                .map(|v| {
                    value_to_u16(v).map_err(|_| {
                        CodecError::Encode(format!(
                            "GeoKeyDirectoryTag (34735) contains non-u16 value: {}",
                            v
                        ))
                    })
                })
                .collect::<Result<Vec<u16>, _>>()?
        }
        None => return Ok((Vec::new(), None, None)),
    };

    // Read optional double params from "34736"
    let double_params = match metadata.get("34736") {
        Some(val) => {
            let arr = val.as_array().ok_or_else(|| {
                CodecError::Encode(
                    "GeoDoubleParamsTag (34736) must be a JSON array of numbers".into(),
                )
            })?;
            let doubles: Result<Vec<f64>, _> = arr
                .iter()
                .map(|v| {
                    v.as_f64().ok_or_else(|| {
                        CodecError::Encode(format!(
                            "GeoDoubleParamsTag (34736) contains non-numeric value: {}",
                            v
                        ))
                    })
                })
                .collect();
            Some(doubles?)
        }
        None => None,
    };

    // Read optional ASCII params from "34737"
    let ascii_params = match metadata.get("34737") {
        Some(val) => {
            let s = val.as_str().ok_or_else(|| {
                CodecError::Encode("GeoAsciiParamsTag (34737) must be a string".into())
            })?;
            Some(s.to_string())
        }
        None => None,
    };

    Ok((directory, double_params, ascii_params))
}

/// Try to convert a serde_json::Value to u16.
fn value_to_u16(val: &Value) -> Result<u16, ()> {
    if let Some(n) = val.as_u64() {
        u16::try_from(n).map_err(|_| ())
    } else if let Some(n) = val.as_i64() {
        u16::try_from(n).map_err(|_| ())
    } else if let Some(n) = val.as_f64() {
        if n.fract() == 0.0 && n >= 0.0 && n <= u16::MAX as f64 {
            Ok(n as u16)
        } else {
            Err(())
        }
    } else {
        Err(())
    }
}

/// Extract transformation tag values from numeric metadata keys.
///
/// Reads `"33550"` (ModelPixelScaleTag), `"33922"` (ModelTiepointTag),
/// and `"34264"` (ModelTransformationTag) from the Tag_Dictionary.
///
/// Returns `(Option<pixel_scale_3>, Option<tiepoints_flat>, Option<transformation_16>)`.
pub fn extract_transformation_tags(
    metadata: &HashMap<String, Value>,
) -> Result<(Option<Vec<f64>>, Option<Vec<f64>>, Option<Vec<f64>>), CodecError> {
    // ModelPixelScaleTag (33550) — exactly 3 DOUBLEs
    let pixel_scale = if let Some(val) = metadata.get("33550") {
        Some(extract_f64_array(
            val,
            "ModelPixelScaleTag (33550)",
            Some(3),
        )?)
    } else {
        None
    };

    // ModelTiepointTag (33922) — flat array, multiple of 6 DOUBLEs
    let tiepoints = if let Some(val) = metadata.get("33922") {
        let arr = extract_f64_array(val, "ModelTiepointTag (33922)", None)?;
        if arr.len() % 6 != 0 {
            return Err(CodecError::Encode(format!(
                "ModelTiepointTag (33922) length must be a multiple of 6, got {}",
                arr.len()
            )));
        }
        Some(arr)
    } else {
        None
    };

    // ModelTransformationTag (34264) — exactly 16 DOUBLEs
    let transformation = if let Some(val) = metadata.get("34264") {
        Some(extract_f64_array(
            val,
            "ModelTransformationTag (34264)",
            Some(16),
        )?)
    } else {
        None
    };

    Ok((pixel_scale, tiepoints, transformation))
}

/// Extract a flat f64 array from a JSON Value, optionally validating length.
fn extract_f64_array(
    val: &Value,
    field_name: &str,
    expected_len: Option<usize>,
) -> Result<Vec<f64>, CodecError> {
    let arr = val.as_array().ok_or_else(|| {
        CodecError::Encode(format!(
            "{} must be a JSON array of exactly {} numbers, got {}",
            field_name,
            expected_len.unwrap_or(0),
            val
        ))
    })?;
    if let Some(expected) = expected_len {
        if arr.len() != expected {
            return Err(CodecError::Encode(format!(
                "{} must be a JSON array of exactly {} numbers, got {}",
                field_name, expected, val
            )));
        }
    }
    arr.iter()
        .map(|v| {
            v.as_f64().ok_or_else(|| {
                CodecError::Encode(format!(
                    "{} must be a JSON array of exactly {} numbers, got {}",
                    field_name,
                    expected_len.unwrap_or(0),
                    val
                ))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // parse_geokeys tests
    // =========================================================================

    /// Helper: build a GeoKey directory with header + entries.
    fn make_directory(keys: &[[u16; 4]]) -> Vec<u16> {
        let mut dir = vec![1, 1, 1, keys.len() as u16];
        for k in keys {
            dir.extend_from_slice(k);
        }
        dir
    }

    #[test]
    fn test_parse_geokeys_inline_short_only() {
        // GTModelTypeGeoKey=1 (Projected), GTRasterTypeGeoKey=1 (PixelIsArea)
        let dir = make_directory(&[[1024, 0, 1, 1], [1025, 0, 1, 1]]);
        let result = parse_geokeys(&dir, None, None).unwrap();
        assert_eq!(result.get("GeoModelType").unwrap(), "Projected");
        assert_eq!(result.get("GeoRasterType").unwrap(), "PixelIsArea");
    }

    #[test]
    fn test_parse_geokeys_double_and_ascii_params() {
        let dir = make_directory(&[
            [1024, 0, 1, 1],     // inline: Projected
            [2048, 0, 1, 4326],  // inline: GeographicCRS
            [4097, 34736, 1, 0], // double param at offset 0
            [4099, 34737, 7, 0], // ASCII param at offset 0, 7 chars
        ]);
        let doubles = [6378137.0];
        let ascii = "WGS 84|";
        let result = parse_geokeys(&dir, Some(&doubles), Some(ascii)).unwrap();

        assert_eq!(result.get("GeoModelType").unwrap(), "Projected");
        assert_eq!(result.get("GeoGeographicCRS").unwrap(), 4326);
        assert_eq!(result.get("GeoKey_4097").unwrap(), 6378137);
        assert_eq!(result.get("GeoKey_4099").unwrap(), "WGS 84");
    }

    #[test]
    fn test_parse_geokeys_model_type_projected() {
        let dir = make_directory(&[[1024, 0, 1, 1]]);
        let result = parse_geokeys(&dir, None, None).unwrap();
        assert_eq!(result.get("GeoModelType").unwrap(), "Projected");
    }

    #[test]
    fn test_parse_geokeys_model_type_geographic() {
        let dir = make_directory(&[[1024, 0, 1, 2]]);
        let result = parse_geokeys(&dir, None, None).unwrap();
        assert_eq!(result.get("GeoModelType").unwrap(), "Geographic");
    }

    #[test]
    fn test_parse_geokeys_raster_type_pixel_is_area() {
        let dir = make_directory(&[[1025, 0, 1, 1]]);
        let result = parse_geokeys(&dir, None, None).unwrap();
        assert_eq!(result.get("GeoRasterType").unwrap(), "PixelIsArea");
    }

    #[test]
    fn test_parse_geokeys_raster_type_pixel_is_point() {
        let dir = make_directory(&[[1025, 0, 1, 2]]);
        let result = parse_geokeys(&dir, None, None).unwrap();
        assert_eq!(result.get("GeoRasterType").unwrap(), "PixelIsPoint");
    }

    #[test]
    fn test_parse_geokeys_projected_cs_numeric() {
        let dir = make_directory(&[[3072, 0, 1, 32618]]);
        let result = parse_geokeys(&dir, None, None).unwrap();
        assert_eq!(result.get("GeoProjectedCRS").unwrap(), 32618);
    }

    #[test]
    fn test_parse_geokeys_geographic_cs_numeric() {
        let dir = make_directory(&[[2048, 0, 1, 4326]]);
        let result = parse_geokeys(&dir, None, None).unwrap();
        assert_eq!(result.get("GeoGeographicCRS").unwrap(), 4326);
    }

    #[test]
    fn test_parse_geokeys_unmapped_key_id() {
        let dir = make_directory(&[[9999, 0, 1, 42]]);
        let result = parse_geokeys(&dir, None, None).unwrap();
        assert_eq!(result.get("GeoKey_9999").unwrap(), 42);
    }

    #[test]
    fn test_parse_geokeys_malformed_directory_too_short() {
        let dir = vec![1, 1, 1]; // only 3 values, need at least 4
        let err = parse_geokeys(&dir, None, None).unwrap_err();
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn test_parse_geokeys_empty_directory() {
        let dir = vec![1, 1]; // only 2 values
        let err = parse_geokeys(&dir, None, None).unwrap_err();
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn test_parse_geokeys_missing_double_params() {
        let dir = make_directory(&[[4097, 34736, 1, 0]]);
        let err = parse_geokeys(&dir, None, None).unwrap_err();
        assert!(err.to_string().contains("34736"));
        assert!(err.to_string().contains("not present"));
    }

    #[test]
    fn test_parse_geokeys_missing_ascii_params() {
        let dir = make_directory(&[[4099, 34737, 5, 0]]);
        let err = parse_geokeys(&dir, None, None).unwrap_err();
        assert!(err.to_string().contains("34737"));
        assert!(err.to_string().contains("not present"));
    }

    #[test]
    fn test_parse_geokeys_double_params_out_of_bounds() {
        let dir = make_directory(&[[4097, 34736, 2, 5]]);
        let doubles = [1.0, 2.0, 3.0];
        let err = parse_geokeys(&dir, Some(&doubles), None).unwrap_err();
        assert!(err.to_string().contains("offset 5"));
    }

    #[test]
    fn test_parse_geokeys_ascii_params_out_of_bounds() {
        let dir = make_directory(&[[4099, 34737, 20, 0]]);
        let err = parse_geokeys(&dir, None, Some("short")).unwrap_err();
        assert!(err.to_string().contains("offset 0"));
        assert!(err.to_string().contains("count 20"));
    }

    #[test]
    fn test_parse_geokeys_zero_keys() {
        let dir = vec![1, 1, 1, 0]; // header with 0 keys
        let result = parse_geokeys(&dir, None, None).unwrap();
        assert!(result.is_empty());
    }

    // =========================================================================
    // build_geokey_directory tests
    // =========================================================================

    #[test]
    fn test_build_empty_metadata() {
        let meta = HashMap::new();
        let (dir, doubles, ascii) = build_geokey_directory(&meta).unwrap();
        assert!(dir.is_empty());
        assert!(doubles.is_none());
        assert!(ascii.is_none());
    }

    #[test]
    fn test_build_roundtrip_inline_keys() {
        // Build raw GeoKey directory: header + 2 keys (ModelType=Projected, ProjectedCRS=32618)
        let mut meta = HashMap::new();
        meta.insert(
            "34735".to_string(),
            serde_json::json!([1, 1, 1, 2, 1024, 0, 1, 1, 3072, 0, 1, 32618]),
        );

        let (dir, doubles, ascii) = build_geokey_directory(&meta).unwrap();
        assert!(doubles.is_none());
        assert!(ascii.is_none());

        // Parse back using the test-only parse_geokeys
        let parsed = parse_geokeys(&dir, None, None).unwrap();
        assert_eq!(parsed.get("GeoModelType").unwrap(), "Projected");
        assert_eq!(parsed.get("GeoProjectedCRS").unwrap(), 32618);
    }

    #[test]
    fn test_build_with_double_and_ascii_params() {
        let mut meta = HashMap::new();
        // Header + 2 keys: one referencing double params, one referencing ASCII params
        meta.insert(
            "34735".to_string(),
            serde_json::json!([1, 1, 1, 2, 4097, 34736, 1, 0, 4099, 34737, 7, 0]),
        );
        meta.insert("34736".to_string(), serde_json::json!([6378137.0]));
        meta.insert("34737".to_string(), serde_json::json!("WGS 84|"));

        let (dir, doubles, ascii) = build_geokey_directory(&meta).unwrap();
        assert_eq!(dir, vec![1, 1, 1, 2, 4097, 34736, 1, 0, 4099, 34737, 7, 0]);
        assert_eq!(doubles.unwrap(), vec![6378137.0]);
        assert_eq!(ascii.unwrap(), "WGS 84|");
    }

    #[test]
    fn test_build_invalid_directory_not_array() {
        let mut meta = HashMap::new();
        meta.insert("34735".to_string(), serde_json::json!("not an array"));
        let err = build_geokey_directory(&meta).unwrap_err();
        assert!(err.to_string().contains("34735"));
        assert!(err.to_string().contains("array"));
    }

    #[test]
    fn test_build_invalid_directory_non_u16_value() {
        let mut meta = HashMap::new();
        // 99999 exceeds u16 range
        meta.insert(
            "34735".to_string(),
            serde_json::json!([1, 1, 1, 1, 1024, 0, 1, 99999]),
        );
        let err = build_geokey_directory(&meta).unwrap_err();
        assert!(err.to_string().contains("non-u16"));
    }

    // =========================================================================
    // extract_transformation_tags tests
    // =========================================================================

    #[test]
    fn test_extract_pixel_scale_valid() {
        let mut meta = HashMap::new();
        meta.insert("33550".to_string(), serde_json::json!([0.5, 0.5, 0.0]));
        let (ps, tp, tf) = extract_transformation_tags(&meta).unwrap();
        assert_eq!(ps.unwrap(), vec![0.5, 0.5, 0.0]);
        assert!(tp.is_none());
        assert!(tf.is_none());
    }

    #[test]
    fn test_extract_pixel_scale_wrong_length() {
        let mut meta = HashMap::new();
        meta.insert("33550".to_string(), serde_json::json!([0.5, 0.5]));
        let err = extract_transformation_tags(&meta).unwrap_err();
        assert!(err.to_string().contains("33550"));
    }

    #[test]
    fn test_extract_tiepoints_valid() {
        let mut meta = HashMap::new();
        meta.insert(
            "33922".to_string(),
            serde_json::json!([0.0, 0.0, 0.0, 300000.0, 4500000.0, 0.0]),
        );
        let (_, tp, _) = extract_transformation_tags(&meta).unwrap();
        assert_eq!(tp.unwrap(), vec![0.0, 0.0, 0.0, 300000.0, 4500000.0, 0.0]);
    }

    #[test]
    fn test_extract_tiepoints_wrong_length() {
        let mut meta = HashMap::new();
        meta.insert(
            "33922".to_string(),
            serde_json::json!([0.0, 0.0, 0.0, 300000.0]),
        );
        let err = extract_transformation_tags(&meta).unwrap_err();
        assert!(err.to_string().contains("33922"));
    }

    #[test]
    fn test_extract_transformation_valid() {
        let mut meta = HashMap::new();
        let vals: Vec<f64> = (0..16).map(|i| i as f64).collect();
        meta.insert(
            "34264".to_string(),
            Value::Array(vals.iter().map(|&v| json_f64(v)).collect()),
        );
        let (_, _, tf) = extract_transformation_tags(&meta).unwrap();
        assert_eq!(tf.unwrap().len(), 16);
    }

    #[test]
    fn test_extract_transformation_wrong_length() {
        let mut meta = HashMap::new();
        meta.insert("34264".to_string(), serde_json::json!([1.0, 2.0, 3.0]));
        let err = extract_transformation_tags(&meta).unwrap_err();
        assert!(err.to_string().contains("34264"));
    }

    #[test]
    fn test_extract_empty_metadata() {
        let meta = HashMap::new();
        let (ps, tp, tf) = extract_transformation_tags(&meta).unwrap();
        assert!(ps.is_none());
        assert!(tp.is_none());
        assert!(tf.is_none());
    }
}
