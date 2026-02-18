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
  - id: reldate
    type: str
    size: 8
    encoding: BCS-N
    doc: |
      Date of Releasability Determination (RELDATE)
      Date when the releasability determination was made by the disclosure
      and release authority/officer. Format CCYYMMDD.
      Unknown values represented with hyphen-minus ("-").
      8 BCS-N characters.

  - id: relslnth
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Releasability Determination Source Length (RELSLNTH)
      Number of bytes in the RELSOURS field.
      Value "0000" indicates no releasability source information.
      4 BCS-N characters, range 0000-9999.

  - id: relsours
    type: str
    size: relslnth.to_i
    encoding: UTF8
    if: relslnth.to_i > 0
    doc: |
      Releasability Determination Source (RELSOURS)
      Name or reference to the releasability determination authority source.
      Variable length, 0-9999 bytes.

  - id: relccslnth
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Country Code Standard Length (RELCCSLNTH)
      Number of bytes in the RELCCSTD field.
      Value "0000" indicates no RELCCSTD information.
      4 BCS-N characters, range 0000-9999.

  - id: relccstd
    type: str
    size: relccslnth.to_i
    encoding: UTF8
    if: relccslnth.to_i > 0
    doc: |
      Country Code Standard (RELCCSTD)
      The country code standard that defines the country codes used in
      the COALCC and RELCCODES fields.
      Variable length, 0-9999 bytes.

  - id: rcolslnth
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Coalition ID Code Standard Length (RCOLSLNTH)
      Number of bytes in the RCOLSTD field.
      Value "0000" indicates no RCOLSTD information.
      4 BCS-N characters, range 0000-9999.

  - id: rcolstd
    type: str
    size: rcolslnth.to_i
    encoding: UTF8
    if: rcolslnth.to_i > 0
    doc: |
      Coalition ID Code Standard (RCOLSTD)
      The coalition ID code standard that defines the coalition codes
      used in the COALID and COALCC fields.
      Variable length, 0-9999 bytes.

  - id: rorgslnth
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Release Organizational Code Standard Length (RORGSLNTH)
      Number of bytes in the RORGSTD field.
      Value "0000" indicates no RORGSTD information.
      4 BCS-N characters, range 0000-9999.

  - id: rorgstd
    type: str
    size: rorgslnth.to_i
    encoding: UTF8
    if: rorgslnth.to_i > 0
    doc: |
      Release Organizational Code Standard (RORGSTD)
      The release organizational code standard that defines the release
      organization codes used in the RELORG field.
      Variable length, 0-9999 bytes.

  - id: coidlnth
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Coalition ID Field Length (COIDLNTH)
      Number of bytes in the COALID field.
      Value "0000" indicates no coalition information.
      4 BCS-N characters, range 0000-9999.

  - id: coalid
    type: str
    size: coidlnth.to_i
    encoding: UTF8
    if: coidlnth.to_i > 0
    doc: |
      Coalition Acronym/Identification (COALID)
      Valid list of coalition multilateral entity codes to which the file
      or segment is authorized for release. Multiple coalitions separated
      by single space (0x20).
      Variable length, 0-9999 bytes.

  - id: coallnth
    type: str
    size: 4
    encoding: BCS-N
    if: coidlnth.to_i > 0
    doc: |
      Coalition Code Field Length (COALLNTH)
      Number of bytes in the COALCC field.
      Value "0000" indicates no coalition nation information.
      4 BCS-N characters, range 0000-9999.
      Only present when COIDLNTH > 0.

  - id: coalcc
    type: str
    size: coallnth.to_i
    encoding: UTF8
    if: coidlnth.to_i > 0 and coallnth.to_i > 0
    doc: |
      Coalition Nations (COALCC)
      Identifies nations in the coalition(s) identified in COALID.
      Coalition nations preceded by coalition acronym, separated from
      national codes by two division sign (0xF7) characters.
      Variable length, 0-9999 bytes.

  - id: relclnth
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Release Countries Field Length (RELCLNTH)
      Number of bytes in the RELCCODES field.
      Value "0000" indicates no country code information.
      4 BCS-N characters, range 0000-9999.

  - id: relccodes
    type: str
    size: relclnth.to_i
    encoding: UTF8
    if: relclnth.to_i > 0
    doc: |
      Country Codes for Releasability (RELCCODES)
      Lists countries to which this NSIF file is releasable.
      Country codes separated by single space (0x20).
      Variable length, 0-9999 bytes.

  - id: rlorglnth
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Release Organizational Code Field Length (RLORGLNTH)
      Number of bytes in the RELORG field.
      Value "0000" indicates no additional organizational information.
      4 BCS-N characters, range 0000-9999.

  - id: relorg
    type: str
    size: rlorglnth.to_i
    encoding: UTF8
    if: rlorglnth.to_i > 0
    doc: |
      List of Organizations (RELORG)
      Free text list of organizations to which imagery can be released.
      Each organization separated by two division sign (0xF7) characters.
      Variable length, 0-9999 bytes.
