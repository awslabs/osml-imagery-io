# Requirements Document

## Introduction

This document specifies the requirements for Phase 0 of the JBP (Joint BIIF Profile) implementation project: the Validation Infrastructure. This foundational phase establishes a Python-based test harness for validating NITF parsing against conformance test files. The infrastructure enables batch execution of tests from any source while respecting data privacy constraints for proprietary test data.

## Glossary

- **JBP**: Joint BIIF Profile - The specification document (JBP-2024.1) defining NITF/NSIF file format requirements
- **Requirement_ID**: A string identifier for specification requirements (e.g., "JBP-2021.2-001")
- **Test_Harness**: A Python/pytest framework for executing validation tests against conformance test files
- **Test_Manifest**: A JSON file defining test files and their expected validation outcomes
- **Test_File_Entry**: A single entry in the manifest describing one test file and its expected outcome

## Requirements

### Requirement 1: Test File Entry Structure

**User Story:** As a developer, I want a generic test file entry structure, so that I can define expected validation outcomes for any test file.

#### Acceptance Criteria

1. THE Test_File_Entry type SHALL include the file path relative to the test data directory
2. THE Test_File_Entry type SHALL include an expected_valid boolean indicating if validation should pass
3. THE Test_File_Entry type SHALL include an optional expected_exception string for the expected exception type name
4. THE Test_File_Entry type SHALL include an optional expected_message string that should appear in the error message
5. THE Test_File_Entry type SHALL include an optional category string for organizational purposes
6. THE Test_File_Entry type SHALL include an optional description string for documentation

### Requirement 2: Test Manifest Management

**User Story:** As a developer, I want to load test expectations from a manifest file, so that expected outcomes are stored separately from code.

#### Acceptance Criteria

1. THE Test_Manifest type SHALL load expected outcomes from a JSON file at a configurable path
2. WHEN the manifest file does not exist, THE system SHALL return an empty manifest without error
3. THE manifest format SHALL be a JSON object with an "entries" array of Test_File_Entry objects
4. WHEN loading the manifest, THE system SHALL validate JSON structure and report parse errors
5. THE Test_Manifest SHALL provide a method to look up expectations for a given file path
6. THE Test_Manifest SHALL provide a method to list all entries

### Requirement 3: Exception Verification

**User Story:** As a developer, I want to verify that specific exceptions are raised for invalid files, so that I can confirm the correct errors are detected.

#### Acceptance Criteria

1. THE Test_Harness SHALL catch exceptions raised by the NITF reader
2. THE Test_Harness SHALL verify the exception type matches expected types when specified
3. THE Test_Harness SHALL verify the exception message contains expected substrings when specified
4. WHEN no specific exception type is expected, THE Test_Harness SHALL accept any exception as a failure

### Requirement 4: Test Harness Execution

**User Story:** As a developer, I want a test harness that validates files against expected outcomes, so that I can verify implementation correctness through the Python API.

#### Acceptance Criteria

1. THE Test_Harness SHALL load test entries from a provided manifest file path
2. WHEN a test file referenced in the manifest does not exist, THE Test_Harness SHALL skip that test and log a warning
3. THE Test_Harness SHALL invoke the NITF reader through the Python API to test the full stack
4. WHEN a test expected to pass raises an exception, THE Test_Harness SHALL report a test failure
5. WHEN a test expected to fail does not raise an exception, THE Test_Harness SHALL report a test failure
6. WHEN expected exception types are specified, THE Test_Harness SHALL verify the exception type matches
7. WHEN expected message patterns are specified, THE Test_Harness SHALL verify the message contains those patterns

### Requirement 5: Pytest Integration

**User Story:** As a developer, I want the test harness to integrate with pytest, so that conformance tests run as part of the standard test suite.

#### Acceptance Criteria

1. THE test harness SHALL generate pytest test cases dynamically from the manifest
2. WHEN a manifest file is not found, THE test harness SHALL skip all generated tests with a warning
3. WHEN a test file referenced in the manifest does not exist, THE test harness SHALL skip that individual test
4. THE test harness SHALL use pytest markers to allow filtering conformance tests (e.g., `pytest -m integration`)
5. THE test harness SHALL report test results using standard pytest assertions
6. THE test harness SHALL support running a subset of tests by category when categories are defined in the manifest

### Requirement 6: Graceful Degradation

**User Story:** As a developer, I want the test harness to handle missing test data gracefully, so that CI/CD pipelines work without proprietary test files.

#### Acceptance Criteria

1. WHEN test data is not present, THE Test_Harness SHALL skip tests and report zero failures
2. WHEN the manifest file is missing, THE system SHALL continue with an empty manifest
3. THE system SHALL log a warning when test data directory is not found
4. IF the OSML_IO_INTEGRATION_DATA environment variable is set, THEN THE system SHALL use that path for test data
5. IF the environment variable is not set, THEN THE system SHALL use the default path "data/integration/"
