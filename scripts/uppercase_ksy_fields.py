#!/usr/bin/env python3
"""
Script to convert lowercase field IDs in .ksy files to uppercase.

This script processes all .ksy files in the data/structures directory and
converts field IDs from lowercase to uppercase to match the JBP specification.

For example:
  - id: im       -> - id: IM
  - id: iid1     -> - id: IID1
  - id: nrows    -> - id: NROWS
"""

import os
import re
from pathlib import Path


def convert_field_id_to_uppercase(content: str) -> str:
    """
    Convert field IDs in a .ksy file from lowercase to uppercase.
    
    This handles:
    - Top-level field IDs: `- id: fieldname`
    - Nested type field IDs: `- id: fieldname` within types section
    - References in expressions: `fieldname.to_i` -> `FIELDNAME.to_i`
    - References in conditions: `if: fieldname != ...` -> `if: FIELDNAME != ...`
    """
    lines = content.split('\n')
    result_lines = []
    
    # Track all field IDs we find so we can update references
    field_ids = set()
    
    # First pass: collect all field IDs (both lowercase and already uppercase)
    for line in lines:
        # Match field ID definitions: `- id: fieldname` or `  - id: fieldname`
        match = re.match(r'^(\s*-\s*id:\s*)([a-zA-Z][a-zA-Z0-9_]*)\s*$', line)
        if match:
            field_id = match.group(2)
            # Store lowercase version for matching
            field_ids.add(field_id.lower())
    
    # Second pass: convert field IDs and update references
    for line in lines:
        # Convert field ID definitions (only lowercase ones)
        match = re.match(r'^(\s*-\s*id:\s*)([a-z][a-z0-9_]*)\s*$', line)
        if match:
            prefix = match.group(1)
            field_id = match.group(2)
            line = f"{prefix}{field_id.upper()}"
        else:
            # Update references in expressions and conditions
            for field_id in field_ids:
                # Match field references in expressions like `fieldname.to_i`
                line = re.sub(
                    rf'\b{field_id}\.to_i\b',
                    f'{field_id.upper()}.to_i',
                    line,
                    flags=re.IGNORECASE
                )
                # Match field references in conditions - handle all comparison operators
                # Pattern: fieldname followed by space and operator
                line = re.sub(
                    rf'\b{field_id}\b(\s*[!=<>])',
                    rf'{field_id.upper()}\1',
                    line,
                    flags=re.IGNORECASE
                )
                # Match field references after "and" or "or" in conditions
                line = re.sub(
                    rf'(and\s+){field_id}\b',
                    rf'\1{field_id.upper()}',
                    line,
                    flags=re.IGNORECASE
                )
                line = re.sub(
                    rf'(or\s+){field_id}\b',
                    rf'\1{field_id.upper()}',
                    line,
                    flags=re.IGNORECASE
                )
        
        result_lines.append(line)
    
    return '\n'.join(result_lines)


def process_ksy_file(filepath: Path) -> bool:
    """
    Process a single .ksy file, converting field IDs to uppercase.
    
    Returns True if the file was modified, False otherwise.
    """
    with open(filepath, 'r', encoding='utf-8') as f:
        original_content = f.read()
    
    converted_content = convert_field_id_to_uppercase(original_content)
    
    if converted_content != original_content:
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(converted_content)
        return True
    return False


def main():
    """Process all .ksy files in data/structures directory."""
    structures_dir = Path('data/structures')
    
    if not structures_dir.exists():
        print(f"Error: {structures_dir} does not exist")
        return 1
    
    modified_count = 0
    total_count = 0
    
    for ksy_file in structures_dir.rglob('*.ksy'):
        total_count += 1
        if process_ksy_file(ksy_file):
            print(f"Modified: {ksy_file}")
            modified_count += 1
        else:
            print(f"Unchanged: {ksy_file}")
    
    print(f"\nProcessed {total_count} files, modified {modified_count}")
    return 0


if __name__ == '__main__':
    exit(main())
