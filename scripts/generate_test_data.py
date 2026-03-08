#!/usr/bin/env python3
"""Generate synthetic test data files for JBP round-trip testing.

This script creates minimal valid NITF/NSIF files using the JBPDatasetWriter
for use in unit tests and round-trip consistency verification.

Generated files:
- data/unit/sample_nitf21.ntf - Minimal NITF 2.1 with one image segment
- data/unit/sample_nsif10.nsif - Minimal NSIF 1.0 with one image segment
- data/unit/multi_segment.ntf - NITF with multiple segment types
"""

import sys
from pathlib import Path

# Add the project root to the path
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

from aws.osml.io import IO, AssetProvider, AssetType


def generate_sample_nitf21(output_path: Path) -> None:
    """Generate a minimal valid NITF 2.1 file with one image segment.
    
    Args:
        output_path: Path to write the output file
    """
    print(f"Generating {output_path}...")
    
    # Create writer for NITF 2.1 format
    writer = IO.open([str(output_path)], "w", "nitf")
    
    # Create a simple 8x8 grayscale image (64 bytes)
    image_data = bytes([(x + y) % 256 for y in range(8) for x in range(8)])
    
    # Create an asset provider from bytes
    image_asset = AssetProvider.from_bytes(
        key="image_segment_0",
        data=image_data,
        asset_type=AssetType.Image,
        title="Sample Image",
        description="A minimal test image",
    )
    
    # Add the image segment
    writer.add_asset(
        key="image_segment_0",
        provider=image_asset,
        title="Sample Image",
        description="A minimal test image",
        roles=["data"]
    )
    
    # Close to write the file
    writer.close()
    
    print(f"  Created {output_path} ({output_path.stat().st_size} bytes)")


def generate_sample_nsif10(output_path: Path) -> None:
    """Generate a minimal valid NSIF 1.0 file with one image segment.
    
    Args:
        output_path: Path to write the output file
    """
    print(f"Generating {output_path}...")
    
    # Create writer for NSIF 1.0 format
    writer = IO.open([str(output_path)], "w", "nsif")
    
    # Create a simple 8x8 grayscale image (64 bytes)
    image_data = bytes([(x + y) % 256 for y in range(8) for x in range(8)])
    
    # Create an asset provider from bytes
    image_asset = AssetProvider.from_bytes(
        key="image_segment_0",
        data=image_data,
        asset_type=AssetType.Image,
        title="Sample Image",
        description="A minimal test image",
    )
    
    # Add the image segment
    writer.add_asset(
        key="image_segment_0",
        provider=image_asset,
        title="Sample Image",
        description="A minimal test image",
        roles=["data"]
    )
    
    # Close to write the file
    writer.close()
    
    print(f"  Created {output_path} ({output_path.stat().st_size} bytes)")


def generate_multi_segment_nitf(output_path: Path) -> None:
    """Generate a NITF file with multiple segments of different types.
    
    Creates a file with:
    - 2 image segments
    - 1 text segment
    - 1 DES segment
    
    Args:
        output_path: Path to write the output file
    """
    print(f"Generating {output_path}...")
    
    # Create writer for NITF 2.1 format
    writer = IO.open([str(output_path)], "w", "nitf")
    
    # Add 2 image segments
    image1_data = bytes([(x + y) % 256 for y in range(16) for x in range(16)])
    image1_asset = AssetProvider.from_bytes(
        key="image_segment_0",
        data=image1_data,
        asset_type=AssetType.Image,
        title="First Image",
        description="First test image (16x16)",
    )
    writer.add_asset(
        key="image_segment_0",
        provider=image1_asset,
        title="First Image",
        description="First test image (16x16)",
        roles=["data"]
    )
    
    image2_data = bytes([(x * y) % 256 for y in range(8) for x in range(8)])
    image2_asset = AssetProvider.from_bytes(
        key="image_segment_1",
        data=image2_data,
        asset_type=AssetType.Image,
        title="Second Image",
        description="Second test image (8x8)",
    )
    writer.add_asset(
        key="image_segment_1",
        provider=image2_asset,
        title="Second Image",
        description="Second test image (8x8)",
        roles=["data"]
    )
    
    # Add 1 text segment
    text_data = b"This is sample text content for testing."
    text_asset = AssetProvider.from_bytes(
        key="text_segment_0",
        data=text_data,
        asset_type=AssetType.Text,
        title="Sample Text",
        description="Test text segment",
    )
    writer.add_asset(
        key="text_segment_0",
        provider=text_asset,
        title="Sample Text",
        description="Test text segment",
        roles=["metadata"]
    )
    
    # Add 1 DES segment
    des_data = b"Sample DES data content"
    des_asset = AssetProvider.from_bytes(
        key="data_segment_0",
        data=des_data,
        asset_type=AssetType.Data,
        title="Sample DES",
        description="Test DES segment",
    )
    writer.add_asset(
        key="data_segment_0",
        provider=des_asset,
        title="Sample DES",
        description="Test DES segment",
        roles=["metadata"]
    )
    
    # Close to write the file
    writer.close()
    
    print(f"  Created {output_path} ({output_path.stat().st_size} bytes)")


def verify_file(file_path: Path) -> bool:
    """Verify that a generated file can be read back.
    
    Args:
        file_path: Path to the file to verify
        
    Returns:
        True if verification passed, False otherwise
    """
    print(f"  Verifying {file_path}...")
    
    try:
        reader = IO.open([str(file_path)], "r")
        keys = reader.get_asset_keys()
        
        print(f"    Found {len(keys)} asset(s): {keys}")
        
        # Verify each asset can be accessed
        for key in keys:
            assert reader.has_asset(key), f"has_asset('{key}') should return True"
            asset = reader.get_asset(key)
            assert asset is not None, f"get_asset('{key}') should return an asset"
            print(f"    - {key}: {asset.media_type}")
        
        print(f"  ✓ Verification passed")
        return True
        
    except Exception as e:
        print(f"  ✗ Verification failed: {e}")
        return False


def main():
    """Generate all synthetic test data files."""
    # Ensure output directory exists
    output_dir = project_root / "data" / "unit"
    output_dir.mkdir(parents=True, exist_ok=True)
    
    print("=" * 60)
    print("Generating synthetic test data files")
    print("=" * 60)
    
    # Generate files
    files_to_generate = [
        (output_dir / "sample_nitf21.ntf", generate_sample_nitf21),
        (output_dir / "sample_nsif10.nsif", generate_sample_nsif10),
        (output_dir / "multi_segment.ntf", generate_multi_segment_nitf),
    ]
    
    success = True
    for output_path, generator_func in files_to_generate:
        try:
            generator_func(output_path)
            if not verify_file(output_path):
                success = False
        except Exception as e:
            print(f"  ✗ Failed to generate {output_path}: {e}")
            import traceback
            traceback.print_exc()
            success = False
    
    print("=" * 60)
    if success:
        print("All files generated and verified successfully!")
        return 0
    else:
        print("Some files failed to generate or verify.")
        return 1


if __name__ == "__main__":
    sys.exit(main())
