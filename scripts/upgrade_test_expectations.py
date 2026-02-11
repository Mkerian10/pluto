#!/usr/bin/env python3
"""
Semi-automated tool to upgrade compile_should_fail() tests to compile_should_fail_with().

Usage:
    python3 scripts/upgrade_test_expectations.py <test_file>

This script:
1. Finds all compile_should_fail() calls in the test file
2. For each one, temporarily modifies it to print the actual error
3. Runs the test to capture the error message
4. Suggests the replacement with compile_should_fail_with()
5. Optionally applies the replacement automatically
"""

import re
import subprocess
import sys
import tempfile
import shutil
from pathlib import Path
from dataclasses import dataclass
from typing import List, Optional

@dataclass
class TestCase:
    """Represents a single test case that needs upgrading."""
    test_name: str
    line_start: int
    line_end: int
    original_code: str
    source_code: str  # The Pluto source being tested

def find_test_cases(file_path: str) -> List[TestCase]:
    """Find all compile_should_fail() test cases in a file."""
    with open(file_path, 'r') as f:
        content = f.read()
        lines = content.split('\n')

    test_cases = []
    i = 0
    current_test_name = None

    while i < len(lines):
        line = lines[i]

        # Look for test function names
        if '#[test]' in line and i + 1 < len(lines):
            next_line = lines[i + 1]
            match = re.search(r'fn\s+(\w+)\s*\(\)', next_line)
            if match:
                current_test_name = match.group(1)

        # Look for compile_should_fail( (without _with)
        if 'compile_should_fail(' in line and 'compile_should_fail_with' not in line:
            # Extract the test case
            start_line = i
            paren_depth = 0
            in_test = False
            source_lines = []

            # Find the extent of the compile_should_fail() call
            j = i
            while j < len(lines):
                test_line = lines[j]

                # Track parentheses to find the end
                for char in test_line:
                    if char == '(':
                        paren_depth += 1
                        in_test = True
                    elif char == ')':
                        paren_depth -= 1

                # Collect source code (between quotes or r#" markers)
                if in_test:
                    source_lines.append(test_line)

                # End of the call
                if in_test and paren_depth == 0:
                    end_line = j
                    break

                j += 1

            if current_test_name:
                original_code = '\n'.join(lines[start_line:end_line + 1])

                # Try to extract the Pluto source code from the test
                # It's usually between quotes or r#" ... "#
                full_text = ' '.join(source_lines)

                # Try r#" first
                source_match = re.search(r'r#"(.*?)"#', full_text, re.DOTALL)
                if not source_match:
                    # Try regular string
                    source_match = re.search(r'"((?:[^"\\]|\\.)*)"', full_text, re.DOTALL)

                source_code = source_match.group(1) if source_match else "<unable to extract>"

                test_cases.append(TestCase(
                    test_name=current_test_name,
                    line_start=start_line,
                    line_end=end_line,
                    original_code=original_code,
                    source_code=source_code
                ))

            i = end_line + 1
        else:
            i += 1

    return test_cases

def get_actual_error(source_code: str) -> Optional[str]:
    """Compile the source code and extract the error message."""
    # Use a temporary file to compile
    with tempfile.NamedTemporaryFile(mode='w', suffix='.pluto', delete=False) as f:
        f.write(source_code)
        temp_path = f.name

    try:
        # Run plutoc compile
        result = subprocess.run(
            ['cargo', 'run', '--', 'compile', temp_path],
            capture_output=True,
            text=True,
            timeout=5
        )

        # Parse error message from stderr
        if result.returncode != 0:
            stderr = result.stderr

            # Look for error message patterns
            # Pluto errors usually have format:  Error: <message>
            # or are rendered with ariadne with [E] prefix

            # Try to extract the core error message
            lines = stderr.split('\n')
            for line in lines:
                line = line.strip()
                # Skip empty lines and file paths
                if not line or line.startswith('[') or '│' in line or '─' in line:
                    continue
                # Look for error indicator
                if 'error:' in line.lower() or 'Error' in line:
                    # Extract message after "error:"
                    if ':' in line:
                        msg = line.split(':', 1)[-1].strip()
                        # Clean up common prefixes
                        msg = msg.replace('[E]', '').strip()
                        return msg

            # Fallback: return first non-empty, non-formatting line
            for line in lines:
                line = line.strip()
                if line and not line.startswith('[') and '│' not in line and '─' not in line:
                    return line

        return None

    except subprocess.TimeoutExpired:
        return "<compilation timeout>"
    except Exception as e:
        return f"<error running compiler: {e}>"
    finally:
        Path(temp_path).unlink(missing_ok=True)

def suggest_replacement(test_case: TestCase, error_msg: str) -> str:
    """Generate the suggested replacement code."""
    # Replace compile_should_fail( with compile_should_fail_with(
    # and add the error message as second argument

    # Find the source code argument
    original = test_case.original_code

    # We need to add a second argument before the closing paren
    # Handle both single line and multi-line cases

    if '\n' not in original:
        # Single line: compile_should_fail("code")
        replacement = original.replace(
            'compile_should_fail(',
            'compile_should_fail_with('
        )
        # Add error message before )
        replacement = replacement.rstrip(');') + f', "{error_msg}");'
    else:
        # Multi-line: need to add error message before the closing )
        lines = original.split('\n')
        last_line_idx = len(lines) - 1

        # Find the line with closing paren
        for i in range(len(lines) - 1, -1, -1):
            if ')' in lines[i]:
                # Insert error message before the )
                lines[i] = lines[i].replace(')', f', "{error_msg}")')
                break

        # Replace compile_should_fail with compile_should_fail_with
        lines[0] = lines[0].replace('compile_should_fail(', 'compile_should_fail_with(')

        replacement = '\n'.join(lines)

    return replacement

def process_file(file_path: str, auto_apply: bool = False):
    """Process a test file and upgrade its test cases."""
    print(f"\nProcessing: {file_path}")
    print("=" * 60)

    test_cases = find_test_cases(file_path)

    if not test_cases:
        print("No compile_should_fail() tests found.")
        return

    print(f"Found {len(test_cases)} test cases to upgrade.\n")

    replacements = []

    for i, tc in enumerate(test_cases, 1):
        print(f"[{i}/{len(test_cases)}] Test: {tc.test_name}")

        # Try to get actual error
        print(f"  Compiling to get error message...")
        error_msg = get_actual_error(tc.source_code)

        if not error_msg:
            print(f"  ⚠️  Could not extract error message (test might not actually fail)")
            continue

        if error_msg.startswith('<'):
            print(f"  ⚠️  {error_msg}")
            continue

        print(f"  Error: {error_msg}")

        # Generate replacement
        replacement = suggest_replacement(tc, error_msg)

        replacements.append((tc, replacement, error_msg))

        if not auto_apply:
            print(f"\n  Original:")
            for line in tc.original_code.split('\n'):
                print(f"    {line}")
            print(f"\n  Suggested:")
            for line in replacement.split('\n'):
                print(f"    {line}")
            print()

    # Apply replacements if requested
    if auto_apply and replacements:
        print(f"\nApplying {len(replacements)} replacements...")

        with open(file_path, 'r') as f:
            content = f.read()
            lines = content.split('\n')

        # Apply in reverse order to maintain line numbers
        for tc, replacement, error_msg in reversed(replacements):
            # Replace the lines
            new_lines = replacement.split('\n')
            lines[tc.line_start:tc.line_end + 1] = new_lines

        # Write back
        with open(file_path, 'w') as f:
            f.write('\n'.join(lines))

        print(f"✓ Applied {len(replacements)} upgrades to {file_path}")

        # Run the test file to verify
        print(f"\nVerifying tests pass...")
        test_name = Path(file_path).stem
        result = subprocess.run(
            ['cargo', 'test', '--test', test_name],
            capture_output=True,
            text=True
        )

        if result.returncode == 0:
            print(f"✓ All tests pass!")
        else:
            print(f"⚠️  Some tests failed. Check output:")
            print(result.stdout)
            print(result.stderr)

    elif not auto_apply:
        print(f"\n{len(replacements)} replacements suggested.")
        print("Run with --apply to automatically apply them.")

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 scripts/upgrade_test_expectations.py <test_file> [--apply]")
        print("\nExample:")
        print("  python3 scripts/upgrade_test_expectations.py tests/integration/traits.rs")
        print("  python3 scripts/upgrade_test_expectations.py tests/integration/traits.rs --apply")
        sys.exit(1)

    file_path = sys.argv[1]
    auto_apply = '--apply' in sys.argv

    if not Path(file_path).exists():
        print(f"Error: File not found: {file_path}")
        sys.exit(1)

    process_file(file_path, auto_apply)

if __name__ == '__main__':
    main()
