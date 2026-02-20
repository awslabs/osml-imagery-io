meta:
  id: tre_frmsga
  title: Framing Array Segment TRE
  endian: be

doc: |
  FRMSGA TRE - Framing Array Segment Tagged Record Extension
  
  Conveys information about an image that was either collected by a framing
  array sensor or derived by compositing frames collected by a framing array
  sensor. The data is formatted using XML.
  
  When specifying a collected frame, FRMSGA provides:
  - Technical capabilities of the sensor
  - Configuration of the sensor and focal plane settings at collection time
  - Sensor array dimensions and pixel sizes
  - Downlink compression and bit depth
  - Scan mode, read type, and focal plane mode
  - Integration time and dynamic range
  
  When specifying a composite frame, FRMSGA provides:
  - Compositing method and information
  - Information about source frames that contributed to the composite
  
  The TRE can be placed in:
  - Image segment subheader: describes the image in that segment
  - File header: describes framing array sensor imagery used in NITF generation
  
  XML root element: "framingArraySegment"
  
  Key XML elements:
  - sensorCharacteristics: Contains filter type
  - sensedFrame: Information about a collected frame (XOR with compositeFrame)
    - numRows, numColumns: Sensor array dimensions
    - pixelHeight, pixelWidth: Pixel dimensions in micrometers
    - downlinkCompressionType, downlinkBitDepth
    - scanMode, readType, focalPlaneMode
    - imageStartTime, imageEndTime, integrationTime
    - dynamicRange, rowAggregation, colAggregation
  - compositeFrame: Information about a composite frame
    - compositeMethod: How frames were combined
    - inputFrames: Source frames used in composite
  
  Reference: STDI-0002 Volume 1, Appendix AN - FRMSGA

seq:
  - id: XML_CONTENT
    type: str
    size-eos: true
    encoding: UTF-8
    doc: |
      XML-encoded framing array segment metadata.
      
      The XML root element is "framingArraySegment" and contains:
      
      Attributes (optional, for security markings):
      - ism:ISMRootNodeAttributeGroup
      - ism:ISMResourceAttributeGroup
      - ism:SecurityAttributesGroup
      
      Elements:
      - ism:NoticeList (optional): Security notices
      - sensorCharacteristics (required):
        - filter: Optical filter type (Open, Broadband, Pan, Red, Blue, etc.)
      - sensedFrame OR compositeFrame (exactly one required):
      
      sensedFrame elements:
      - IID1, IID2: Image segment identifiers (optional)
      - numRows, numColumns: Sensor array size in pixels
      - windowRowOrigin, windowColumnOrigin: Window origin (optional)
      - numRowsWindow, numColumnsWindow: Window size (optional)
      - pixelHeight, pixelWidth: Pixel dimensions in micrometers
      - drivingSensorFlag: Boolean (optional)
      - downlinkCompressionType: Compression type token
      - downlinkBitDepth: Bit depth as decimal
      - scanMode: Scan mode token
      - readType: Read type token
      - focalPlaneMode: Focal plane mode token
      - focalPlaneQuantization: Quantization type (optional)
      - focalPlaneQuantizationBitDepth: Quantization bit depth (optional)
      - wellDepth: Well depth in electrons (optional)
      - columnAmplifierGain: Gain value
      - imageStartTime, imageEndTime: UTC timestamps
      - integrationTime: Integration time in seconds
      - dynamicRange: Dynamic range in dB (optional)
      - rowAggregation, colAggregation: Aggregation factors (optional)
      
      compositeFrame elements:
      - IID1, IID2: Image segment identifiers (optional)
      - compositeMethod: Method used to combine frames
      - dynamicRangeScalingMode: Scaling mode (optional)
      - dynamicRange: Dynamic range in dB (optional)
      - inputFrames: Container for source sensedFrame elements

