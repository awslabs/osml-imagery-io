meta:
  id: tre_j2klra
  title: JPEG 2000 Layers TRE
  endian: be

doc: |
  J2KLRA TRE - Joint Photographic Experts Group 2000 Layers Tagged Record Extension
  
  Provides information about JPEG 2000 compression parameters including
  resolution levels, quality layers, and bands for both original and
  derived products. This TRE is used with NSIF Preferred JPEG 2000
  Encoding (NPJE) data to enable quick access to compressed data.
  
  The TRE contains:
  - Original image parameters (levels, bands, layers)
  - Per-layer information (bitrate)
  
  Variable length TRE (minimum 19 bytes + 9 bytes per layer)
  
  Reference: STDI-0002 Volume 1, Appendix Y - J2KLRA
  Reference: BPJ2K01.10 - BIIF Profile for JPEG 2000 Version 01.10

seq:
  - id: ORIG_L
    type: str
    size: 1
    encoding: BCS-N
    doc: |
      Original Number of Wavelet Decomposition Levels.
      Number of decomposition levels in the original image.
      Range: 0-9. Value 0 indicates only the LL subband.

  - id: NLEVELS_O
    type: str
    size: 2
    encoding: BCS-N
    doc: |
      Number of Discrete Wavelet Levels in this Image.
      Number of decomposition levels in this (possibly derived) image.
      Range: 00-32.

  - id: NBANDS_O
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Number of Image Components (Bands) in this Image.
      Range: 00001-16384.

  - id: NLAYERS_O
    type: str
    size: 3
    encoding: BCS-N
    doc: |
      Number of Quality Layers in this Image.
      Range: 001-999.

  - id: LAYERS
    type: layer_info
    repeat: expr
    repeat-expr: NLAYERS_O.to_i
    doc: Per-layer information for each quality layer.

types:
  layer_info:
    doc: Information for a single quality layer.
    seq:
      - id: LAYER_ID
        type: str
        size: 3
        encoding: BCS-N
        doc: |
          Layer ID.
          Sequential layer number starting from 001.
          Range: 001-999.

      - id: BITRATE
        type: str
        size: 9
        encoding: BCS-N
        doc: |
          Bitrate for this Layer.
          Cumulative bits per pixel per band for this layer.
          Format: XXXX.XXXX (4 digits, decimal point, 4 digits).
          Range: 0000.0000 to 9999.9999.
          Value 0000.0000 indicates lossless layer.

