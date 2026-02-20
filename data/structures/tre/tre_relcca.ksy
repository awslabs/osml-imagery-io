meta:
  id: tre_relcca
  title: Releasability TRE
  endian: be

doc: |
  RELCCA TRE - Releasability Tagged Record Extension
  
  Provides additional character space to augment the xSREL field (releasing
  instructions) of a NITF file. Allows for longer lists of country codes and
  multilateral entity codes, plus listing of organizations to which imagery
  can be released.
  
  The TRE is organized into three sections:
  - Coalition group (COALID, COALCC)
  - Country group (RELCCODES)
  - Organization group (RELORG)
  
  Reference: STDI-0002 Volume 1, Appendix AD - RELCCA

seq:
  - id: RELDATE
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Date of Releasability Determination (RELDATE)
      Date when the releasability determination was made by the disclosure
      and release authority/officer. Format CCYYMMDD.
      Unknown values represented with hyphen-minus ("-").
      8 BCS-N characters.

  - id: RELSLNTH
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Releasability Determination Source Length (RELSLNTH)
      Number of bytes in the RELSOURS field.
      Value "0000" indicates no releasability source information.
      4 BCS-N characters, range 0000-9999.

  - id: RELSOURS
    type: str
    size: RELSLNTH.to_i
    encoding: UTF8
    if: RELSLNTH.to_i > 0
    doc: |
      Releasability Determination Source (RELSOURS)
      Name or reference to the releasability determination authority source.
      Variable length, 0-9999 bytes.

  - id: RELCCSLNTH
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Country Code Standard Length (RELCCSLNTH)
      Number of bytes in the RELCCSTD field.
      Value "0000" indicates no RELCCSTD information.
      4 BCS-N characters, range 0000-9999.

  - id: RELCCSTD
    type: str
    size: RELCCSLNTH.to_i
    encoding: UTF8
    if: RELCCSLNTH.to_i > 0
    doc: |
      Country Code Standard (RELCCSTD)
      The country code standard that defines the country codes used in
      the COALCC and RELCCODES fields.
      Variable length, 0-9999 bytes.

  - id: RCOLSLNTH
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Coalition ID Code Standard Length (RCOLSLNTH)
      Number of bytes in the RCOLSTD field.
      Value "0000" indicates no RCOLSTD information.
      4 BCS-N characters, range 0000-9999.

  - id: RCOLSTD
    type: str
    size: RCOLSLNTH.to_i
    encoding: UTF8
    if: RCOLSLNTH.to_i > 0
    doc: |
      Coalition ID Code Standard (RCOLSTD)
      The coalition ID code standard that defines the coalition codes
      used in the COALID and COALCC fields.
      Variable length, 0-9999 bytes.

  - id: RORGSLNTH
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Release Organizational Code Standard Length (RORGSLNTH)
      Number of bytes in the RORGSTD field.
      Value "0000" indicates no RORGSTD information.
      4 BCS-N characters, range 0000-9999.

  - id: RORGSTD
    type: str
    size: RORGSLNTH.to_i
    encoding: UTF8
    if: RORGSLNTH.to_i > 0
    doc: |
      Release Organizational Code Standard (RORGSTD)
      The release organizational code standard that defines the release
      organization codes used in the RELORG field.
      Variable length, 0-9999 bytes.

  - id: COIDLNTH
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Coalition ID Field Length (COIDLNTH)
      Number of bytes in the COALID field.
      Value "0000" indicates no coalition information.
      4 BCS-N characters, range 0000-9999.

  - id: COALID
    type: str
    size: COIDLNTH.to_i
    encoding: UTF8
    if: COIDLNTH.to_i > 0
    doc: |
      Coalition Acronym/Identification (COALID)
      Valid list of coalition multilateral entity codes to which the file
      or segment is authorized for release. Multiple coalitions separated
      by single space (0x20).
      Variable length, 0-9999 bytes.

  - id: COALLNTH
    type: str
    size: 4
    encoding: BCS-N
    if: COIDLNTH.to_i > 0
    doc: |
      Coalition Code Field Length (COALLNTH)
      Number of bytes in the COALCC field.
      Value "0000" indicates no coalition nation information.
      4 BCS-N characters, range 0000-9999.
      Only present when COIDLNTH > 0.

  - id: COALCC
    type: str
    size: COALLNTH.to_i
    encoding: UTF8
    if: COIDLNTH.to_i > 0 and COALLNTH.to_i > 0
    doc: |
      Coalition Nations (COALCC)
      Identifies nations in the coalition(s) identified in COALID.
      Coalition nations preceded by coalition acronym, separated from
      national codes by two division sign (0xF7) characters.
      Variable length, 0-9999 bytes.

  - id: RELCLNTH
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Release Countries Field Length (RELCLNTH)
      Number of bytes in the RELCCODES field.
      Value "0000" indicates no country code information.
      4 BCS-N characters, range 0000-9999.

  - id: RELCCODES
    type: str
    size: RELCLNTH.to_i
    encoding: UTF8
    if: RELCLNTH.to_i > 0
    doc: |
      Country Codes for Releasability (RELCCODES)
      Lists countries to which this NSIF file is releasable.
      Country codes separated by single space (0x20).
      Variable length, 0-9999 bytes.

  - id: RLORGLNTH
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Release Organizational Code Field Length (RLORGLNTH)
      Number of bytes in the RELORG field.
      Value "0000" indicates no additional organizational information.
      4 BCS-N characters, range 0000-9999.

  - id: RELORG
    type: str
    size: RLORGLNTH.to_i
    encoding: UTF8
    if: RLORGLNTH.to_i > 0
    doc: |
      List of Organizations (RELORG)
      Free text list of organizations to which imagery can be released.
      Each organization separated by two division sign (0xF7) characters.
      Variable length, 0-9999 bytes.
