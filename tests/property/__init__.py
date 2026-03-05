# Property-based testing module for osml-imagery-io
#
# This module contains property-based tests that validate universal correctness
# properties across generated test inputs. Property tests complement unit tests
# by verifying that properties hold for all valid inputs, not just specific examples.
#
# Structure:
#   - strategies.py: Reusable hypothesis strategies for image/block/metadata generation
#   - quality.py: PSNR/SSIM calculation for lossy compression validation
#   - conftest.py: Shared fixtures and pytest configuration
#   - test_*.py: Property test modules organized by feature area
