meta:
  id: tre_illuma
  title: Illumination TRE (XML-encoded)
  endian: be

doc: |
  ILLUMA TRE - Illumination Tagged Record Extension (XML-encoded)
  
  Provides illumination metadata encoded using XML. Contains information
  about natural and artificial illumination relevant at the time and
  location of data collection for electro-optical imagery.
  
  The ILLUMA TRE contains:
  - Solar azimuth and elevation angles
  - Computed solar illumination (radiance)
  - Lunar azimuth and elevation angles
  - Lunar phase angle
  - Computed lunar illumination (radiance)
  - Solar/lunar distance adjustment factor
  - Computed total natural illumination
  - Minimum and maximum artificial illumination estimates
  
  Note: ILLUMA uses XML encoding, so the content is variable-length
  XML data. This definition captures the raw XML content which must
  be parsed separately.
  
  Reference: STDI-0002 Volume 1, Appendix AL - ILLUMA-ILLUMB

seq:
  - id: xml_content
    type: str
    size-eos: true
    encoding: UTF-8
    doc: |
      XML-encoded illumination metadata.
      The XML root element is "ILLUMA" and contains optional elements:
      - solAz: Solar azimuth angle (0.0 to 359.9 degrees)
      - solEl: Solar elevation angle (-90.0 to +90.0 degrees)
      - comSolIl: Computed solar illumination (W m^-2 sr^-1)
      - lunAz: Lunar azimuth angle (0.0 to 359.9 degrees)
      - lunEl: Lunar elevation angle (-90.0 to +90.0 degrees)
      - lunPhAng: Lunar phase angle (-180.0 to +180.0 degrees)
      - comLunIl: Computed lunar illumination (W m^-2 sr^-1)
      - solLunDisAd: Solar/lunar distance adjustment (0.70000 to 1.40000)
      - comTotNatIl: Computed total natural illumination (W m^-2 sr^-1)
      - artIlMin: Minimum artificial illumination (W m^-2 sr^-1)
      - artIlMax: Maximum artificial illumination (W m^-2 sr^-1)
      - extensionPoint: Reserved for future use

