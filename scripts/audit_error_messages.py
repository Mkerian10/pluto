#!/usr/bin/env python3
"""
Extract and analyze all error messages from the Pluto compiler source code.
Generates a comprehensive audit report with quality ratings.
"""

import re
import os
import csv
from collections import defaultdict
from pathlib import Path
from dataclasses import dataclass
from typing import List, Dict

@dataclass
class ErrorSite:
    file: str
    line: int
    error_type: str
    message: str
    full_context: str
    quality: str = "Unknown"
    notes: str = ""

def extract_error_message(line: str, following_lines: List[str]) -> str:
    """Extract the error message from a CompileError callsite, handling multi-line formats."""
    # Try to find the message in the current line first
    patterns = [
        r'CompileError::\w+\(\s*format!\s*\(\s*"([^"]+)"',  # format!("msg"
        r'CompileError::\w+\(\s*"([^"]+)"',  # "msg"
        r'CompileError::\w+\(\s*([a-zA-Z_][a-zA-Z0-9_]*)',  # variable
    ]

    for pattern in patterns:
        match = re.search(pattern, line)
        if match:
            return match.group(1)

    # Check if it's a multi-line format! call
    if 'format!' in line and '"' not in line:
        for next_line in following_lines[:3]:  # Look ahead up to 3 lines
            msg_match = re.search(r'"([^"]+)"', next_line)
            if msg_match:
                return msg_match.group(1)

    return "<complex or variable message>"

def get_error_type(line: str) -> str:
    """Extract the error type (syntax, type_err, codegen, etc.)"""
    match = re.search(r'CompileError::(\w+)\(', line)
    return match.group(1) if match else "unknown"

def rate_error_quality(message: str, file: str) -> tuple:
    """
    Rate error message quality based on criteria:
    - Excellent: Contextual + actionable, shows types/names, suggests fixes
    - Good: Specific with types/names
    - Adequate: Clear but generic
    - Poor: Empty, unhelpful, or just "error"
    """

    # Poor quality indicators
    if message in ["<complex or variable message>", "error", ""]:
        return ("Poor", "Generic or missing message")

    # Check for variables/patterns in format strings
    has_types = any(x in message for x in ["{", "expected", "found", "type"])
    has_names = any(x in message for x in ["'{", "variable", "field", "class", "function"])
    has_suggestion = any(x in message for x in ["; ", "add ", "use ", "declare", "remove"])
    has_operation = any(x in message for x in ["cannot", "requires", "must", "operator"])

    # Excellent: Multiple criteria met
    if sum([has_types, has_names, has_suggestion, has_operation]) >= 3:
        return ("Excellent", "Contextual, specific, and actionable")

    # Good: Shows specific information
    if has_types and has_names:
        return ("Good", "Shows types and names")
    if has_types or has_names:
        return ("Good", "Includes specific information")

    # Adequate: Clear but generic
    if has_operation or len(message) > 20:
        return ("Adequate", "Clear but could be more specific")

    return ("Adequate", "Basic error message")

def extract_all_errors() -> List[ErrorSite]:
    """Walk through src/ and extract all CompileError callsites."""
    errors = []

    for root, dirs, files in os.walk('src'):
        for filename in files:
            if not filename.endswith('.rs'):
                continue

            filepath = os.path.join(root, filename)

            with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
                lines = f.readlines()

            for i, line in enumerate(lines):
                if 'CompileError::' not in line:
                    continue

                # Skip test code and error type definitions
                if 'fn test_' in line or 'match' in line or '|' in line:
                    # Check if this is pattern matching on error types
                    if re.search(r'CompileError::\w+\s*{', line):
                        continue

                error_type = get_error_type(line)
                if not error_type or error_type == "unknown":
                    continue

                # Get following lines for multi-line extraction
                following = lines[i+1:i+4] if i+1 < len(lines) else []
                message = extract_error_message(line, following)

                # Get context (the full statement, possibly multi-line)
                context_lines = [line.strip()]
                j = i + 1
                while j < len(lines) and j < i + 5:
                    next_line = lines[j].strip()
                    context_lines.append(next_line)
                    if ';' in next_line or '}' in next_line:
                        break
                    j += 1

                full_context = ' '.join(context_lines[:3])  # First 3 lines max

                quality, notes = rate_error_quality(message, filepath)

                errors.append(ErrorSite(
                    file=filepath,
                    line=i + 1,
                    error_type=error_type,
                    message=message,
                    full_context=full_context[:200],  # Limit context length
                    quality=quality,
                    notes=notes
                ))

    return errors

def generate_report(errors: List[ErrorSite], output_file: str):
    """Generate CSV report of all error messages."""
    with open(output_file, 'w', newline='', encoding='utf-8') as f:
        writer = csv.writer(f)
        writer.writerow(['File', 'Line', 'Error Type', 'Quality', 'Message', 'Notes', 'Context'])

        for error in sorted(errors, key=lambda e: (e.quality, e.file, e.line)):
            writer.writerow([
                error.file,
                error.line,
                error.error_type,
                error.quality,
                error.message,
                error.notes,
                error.full_context
            ])

def print_summary(errors: List[ErrorSite]):
    """Print summary statistics."""
    total = len(errors)
    by_quality = defaultdict(int)
    by_file = defaultdict(int)
    by_type = defaultdict(int)

    for error in errors:
        by_quality[error.quality] += 1
        by_file[error.file] += 1
        by_type[error.error_type] += 1

    print(f"\n{'='*60}")
    print(f"ERROR MESSAGE AUDIT SUMMARY")
    print(f"{'='*60}\n")

    print(f"Total error generation sites: {total}\n")

    print("Quality Distribution:")
    for quality in ['Excellent', 'Good', 'Adequate', 'Poor']:
        count = by_quality.get(quality, 0)
        pct = (count / total * 100) if total > 0 else 0
        print(f"  {quality:12s}: {count:3d} ({pct:5.1f}%)")

    print(f"\nBy Error Type:")
    for error_type, count in sorted(by_type.items(), key=lambda x: -x[1])[:10]:
        print(f"  {error_type:15s}: {count:3d}")

    print(f"\nTop Files (by error count):")
    for file, count in sorted(by_file.items(), key=lambda x: -x[1])[:10]:
        print(f"  {file:40s}: {count:3d}")

    print(f"\n{'='*60}\n")

if __name__ == '__main__':
    print("Extracting error messages from Pluto compiler source...")

    errors = extract_all_errors()

    output_file = 'analysis/error_message_audit.csv'
    os.makedirs('analysis', exist_ok=True)
    generate_report(errors, output_file)

    print(f"Audit report written to: {output_file}")

    print_summary(errors)

    # Find examples of each quality level
    print("\nSample messages by quality:\n")

    for quality in ['Excellent', 'Good', 'Adequate', 'Poor']:
        examples = [e for e in errors if e.quality == quality][:3]
        if examples:
            print(f"{quality}:")
            for ex in examples:
                print(f"  - {ex.message[:80]}")
                print(f"    ({ex.file}:{ex.line})")
            print()
