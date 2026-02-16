# Implementation Plan: JBP Validation Infrastructure

## Overview

This implementation plan creates a Python-based test harness for validating NITF parsing against conformance test files. The infrastructure uses pytest for test execution and supports manifest-driven test case generation with graceful degradation when test data is unavailable.

## Tasks

- [x] 1. Create TestFileEntry and TestManifest data classes
  - [x] 1.1 Create `tests/conformance/__init__.py` with TestFileEntry dataclass
    - Define fields: path, expected_valid, expected_exception, expected_message, category, description
    - Use dataclasses with default values for optional fields
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_
  - [x] 1.2 Add TestManifest class with load/save methods
    - Implement `load()` classmethod that returns empty manifest if file not found
    - Implement `to_json()` and `from_json()` methods for serialization
    - Implement `get_entry()` for path lookup
    - Implement `entries_by_category()` for filtering
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_
  - [x] 1.3 Write unit tests for TestFileEntry and TestManifest
    - Test manifest loading with missing file
    - Test manifest loading with valid JSON
    - Test entry lookup found/not found
    - Test JSON parse error handling
    - _Requirements: 2.2, 2.4_

- [ ] 2. Implement property-based tests for manifest
  - [ ]* 2.1 Write property test for manifest JSON round-trip
    - **Property 1: Manifest JSON Round-Trip**
    - **Validates: Requirements 2.3**
  - [ ]* 2.2 Write property test for manifest lookup
    - **Property 2: Manifest Lookup Returns Correct Entry**
    - **Validates: Requirements 2.5**

- [x] 3. Create pytest conformance test infrastructure
  - [x] 3.1 Add helper functions for integration data path resolution
    - Implement `get_integration_data_path()` using env var or default
    - Implement `get_manifest_path()` for manifest location
    - _Requirements: 6.4, 6.5_
  - [x] 3.2 Create `tests/test_conformance.py` with parametrized test
    - Implement `load_test_cases()` for pytest parametrization
    - Create `test_conformance()` with `@pytest.mark.integration` marker
    - Handle missing test files with `pytest.skip()`
    - Implement pass/fail logic based on expected_valid
    - Implement exception type verification when expected_exception is set
    - Implement message substring verification when expected_message is set
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 5.1, 5.2, 5.3, 5.4, 5.5_
  - [ ]* 3.3 Write property tests for pass/fail determination logic
    - **Property 3: Test Pass/Fail Determination Is Correct**
    - **Validates: Requirements 4.4, 4.5**
  - [ ]* 3.4 Write property tests for exception verification logic
    - **Property 4: Exception Type Matching Is Correct**
    - **Property 5: Message Substring Matching Is Correct**
    - **Validates: Requirements 4.6, 4.7**

- [x] 4. Configure pytest markers and graceful degradation
  - [x] 4.1 Update `pyproject.toml` or `pytest.ini` with integration marker
    - Add `integration` marker definition
    - _Requirements: 5.4_
  - [x] 4.2 Add logging for missing test data scenarios
    - Log warning when manifest file not found
    - Log warning when test file not found (before skip)
    - _Requirements: 6.1, 6.2, 6.3_

- [x] 5. Checkpoint - Verify test infrastructure works
  - Create a sample manifest in a temp directory
  - Run pytest to verify parametrization works
  - Verify tests skip gracefully when data is missing
  - Ensure all unit and property tests pass

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- No Rust code changes are needed for Phase 0
- The manifest file (`data/integration/manifest.json`) is gitignored and must be created by users with test data access
- Property tests use hypothesis with minimum 100 iterations
