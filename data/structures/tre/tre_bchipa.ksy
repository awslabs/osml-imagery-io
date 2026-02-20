meta:
  id: tre_bchipa
  title: Band Chipping TRE
  endian: be

doc: |
  BCHIPA TRE - Band Chipping Support Data Extension
  
  Records the parsing, reordering, and/or combination of bands that has been
  applied to image data. Provides mapping between current image bands and
  original bands, similar to how ICHIPB provides spatial chipping information.
  
  This is a complex TRE with conditional sections and variable-length fields.
  Multiple instances may be required to contain all support data for a
  band-wise processed image.
  
  NOTE: This is a simplified definition that captures the fixed header fields.
  The full TRE has complex conditional logic with nested parent references
  that requires runtime evaluation. Conditional sections are captured as
  raw bytes in the conditional_data field.
  
  Variable length TRE (minimum 68 bytes, maximum 99985 bytes)
  
  Reference: STDI-0002 Volume 1, Appendix AR - BCHIPA

seq:
  - id: SDE_UUID
    type: str
    size: 36
    encoding: BCS-A
    doc: |
      UUID assigned to the series of BCHIPA TREs associated with the image.
      Canonical format using lower-case characters, or all blank spaces (0x20)
      if no UUID is provided.

  - id: NUM_INSTS
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Number of instances of BCHIPA TRE associated with this band-wise
      processed image. Range: 00001-99999

  - id: INSTANCE
    type: str
    size: 5
    encoding: BCS-N
    doc: |
      Current instance number within the series of BCHIPA TREs.
      Range: 00001-99999

  - id: INCLUDE_A
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Inclusion flag for image segment identification and relevant SDE info.
      Y = include fields, N = exclude fields.
      Must be Y if INSTANCE = 00001.

  - id: INCLUDE_B
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Inclusion flag for original band information.
      Y = include fields, N = exclude fields.

  - id: INCLUDE_C
    type: str
    size: 1
    encoding: BCS-A
    doc: |
      Inclusion flag for band correspondence information.
      Y = include fields, N = exclude fields.

  # Remaining data depends on include flags and has complex nested conditionals
  # This simplified definition captures the raw remaining bytes
  - id: CONDITIONAL_DATA
    size-eos: true
    doc: |
      Conditional sections based on include flags (A, B, C).
      Section A: Image segment identification and relevant SDE information
      Section B: Original band information with LUT data
      Section C: Band correspondence/mapping information with formulas
      Full parsing requires runtime evaluation of nested conditionals.
