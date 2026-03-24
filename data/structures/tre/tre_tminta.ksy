meta:
  id: tre_tminta
  title: Time Interval TRE
  endian: be

doc: |
  TMINTA TRE - Time Interval Tagged Record Extension

  Defines the start and end times for one or more time intervals. Multiple
  TMINTA TREs may be used to "on-the-fly" define time intervals as data is
  being collected.

  The TMINTA containing the time interval definitions for the MI data within
  a file must reside in that file. A given file may also have TMINTA TREs
  from other files that correspond to one or more "temporally adjacent"
  time intervals.

  Time stamps are given in UTC with fractional time to the nanosecond using
  a 24-character BCS-A timestamp format.

  This TRE is part of the Motion Imagery Extensions for NITF 2.1 (MIE4NITF)
  specification defined in NGA.STND.0044.

  Reference: STDI-0002 Volume 1, Appendix AF, Section AF 5.4
  Reference: NGA.STND.0044_1.3.3 - Motion Imagery Extension for NITF 2.1

seq:
  - id: NUM_TIME_INT
    type: str
    size: 4
    encoding: BCS-N
    doc: |
      Number of Time Intervals
      4 BCS-N positive integer.

  - id: TIME_INTERVALS
    type: time_interval_record
    repeat: expr
    repeat-expr: NUM_TIME_INT.to_i
    doc: Time interval definitions.

types:
  time_interval_record:
    seq:
      - id: TIME_INTERVAL_INDEX
        type: str
        size: 6
        encoding: BCS-N
        doc: |
          Time Interval Index
          6 BCS-N positive integer. 1-based index of this time interval.

      - id: START_TIMESTAMP
        type: str
        size: 24
        encoding: BCS-A
        doc: |
          Start Timestamp
          24 BCS-A. UTC timestamp (YYYYMMDDHHmmSS.fffffffff---).
          All spaces if the time interval is empty/does not exist.

      - id: END_TIMESTAMP
        type: str
        size: 24
        encoding: BCS-A
        doc: |
          End Timestamp
          24 BCS-A. UTC timestamp (YYYYMMDDHHmmSS.fffffffff---).
          All spaces if the time interval is empty/does not exist.
