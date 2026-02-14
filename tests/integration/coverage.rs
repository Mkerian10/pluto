mod common;

use plutoc::coverage::*;
use plutoc::modules::SourceMap;
use std::path::PathBuf;

// ── Coverage map scanning tests ─────────────────────────────────────────────

fn make_source_map(source: &str) -> SourceMap {
    let mut sm = SourceMap::new();
    sm.add_file(PathBuf::from("test.pluto"), source.to_string());
    sm
}

fn build_map(source: &str) -> CoverageMap {
    let sm = make_source_map(source);
    build_coverage_map(
        &plutoc::parse_source(source).unwrap(),
        &sm,
    )
}

#[test]
fn coverage_map_simple_function() {
    let map = build_map(
        r#"
fn main() {
    let x = 42
    print(x)
}
"#,
    );
    // Should have function entry + 2 statements
    assert!(map.num_points() >= 3, "expected at least 3 points, got {}", map.num_points());
    assert_eq!(map.files.len(), 1);
    assert_eq!(map.files[0].path, "test.pluto");

    let entries: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::FunctionEntry).collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].function_name, "main");
}

#[test]
fn coverage_map_multiple_functions() {
    let map = build_map(
        r#"
fn add(a: int, b: int) int {
    return a + b
}
fn main() {
    let result = add(1, 2)
    print(result)
}
"#,
    );
    let entries: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::FunctionEntry).collect();
    assert_eq!(entries.len(), 2);
    let names: Vec<_> = entries.iter().map(|e| e.function_name.as_str()).collect();
    assert!(names.contains(&"add"));
    assert!(names.contains(&"main"));
}

#[test]
fn coverage_map_if_else_branches() {
    let map = build_map(
        r#"
fn main() {
    let x = 42
    if x > 10 {
        print(x)
    } else {
        print(0)
    }
}
"#,
    );
    // Function entry + let + if + then body stmt + else body stmt
    let stmts: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::Statement).collect();
    // At minimum: let, if, print(x), print(0) = 4 statements
    assert!(stmts.len() >= 4, "expected at least 4 statements, got {}", stmts.len());
}

#[test]
fn coverage_map_while_loop() {
    let map = build_map(
        r#"
fn main() {
    let mut i = 0
    while i < 10 {
        i = i + 1
    }
}
"#,
    );
    // Should have points inside the loop body
    let stmts: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::Statement).collect();
    assert!(stmts.len() >= 3, "expected at least 3 statements (let, while, assign), got {}", stmts.len());
}

#[test]
fn coverage_map_class_methods() {
    let map = build_map(
        r#"
class Counter {
    value: int

    fn increment(mut self) {
        self.value = self.value + 1
    }

    fn get(self) int {
        return self.value
    }
}
fn main() {
    let mut c = Counter { value: 0 }
    c.increment()
    print(c.get())
}
"#,
    );
    let entries: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::FunctionEntry).collect();
    let names: Vec<_> = entries.iter().map(|e| e.function_name.as_str()).collect();
    assert!(names.contains(&"Counter.increment"), "missing Counter.increment, got {:?}", names);
    assert!(names.contains(&"Counter.get"), "missing Counter.get, got {:?}", names);
    assert!(names.contains(&"main"), "missing main, got {:?}", names);
}

#[test]
fn coverage_map_match_arms() {
    let map = build_map(
        "enum Color {\n    Red\n    Green\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Color.Red {\n            print(1)\n        }\n        Color.Green {\n            print(2)\n        }\n        Color.Blue {\n            print(3)\n        }\n    }\n}\n",
    );
    // Match arms should each have their own coverage points
    let stmts: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::Statement).collect();
    // At minimum: let, match, 3 arm bodies
    assert!(stmts.len() >= 5, "expected at least 5 statements, got {}", stmts.len());
}

#[test]
fn coverage_map_ids_are_sequential() {
    let map = build_map(
        r#"
fn main() {
    let a = 1
    let b = 2
    let c = 3
}
"#,
    );
    for (i, point) in map.points.iter().enumerate() {
        assert_eq!(point.id as usize, i, "point IDs should be sequential");
    }
}

#[test]
fn coverage_map_line_numbers_correct() {
    let source = "fn main() {\n    let x = 1\n    let y = 2\n}\n";
    let sm = make_source_map(source);
    let map = build_coverage_map(
        &plutoc::parse_source(source).unwrap(),
        &sm,
    );
    // Find the statement for `let x = 1` (line 2)
    let stmts: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::Statement).collect();
    assert!(stmts.len() >= 2);
    // First statement should be on line 2 (let x = 1)
    assert_eq!(stmts[0].line, 2, "first stmt should be on line 2");
    // Second statement should be on line 3 (let y = 2)
    assert_eq!(stmts[1].line, 3, "second stmt should be on line 3");
}

// ── Span lookup tests ───────────────────────────────────────────────────────

#[test]
fn span_lookup_matches_points() {
    let map = build_map("fn main() {\n    let x = 1\n}\n");
    let lookup = map.build_span_lookup();
    for point in &map.points {
        assert_eq!(lookup.get(&(point.file_id, point.byte_offset, point.branch_id)), Some(&point.id));
    }
}

// ── Coverage data (binary format) tests ─────────────────────────────────────

#[test]
fn coverage_data_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let data_path = dir.path().join("coverage-data.bin");

    // Write a binary counter file manually
    let num_points: i64 = 3;
    let counters: Vec<i64> = vec![10, 0, 5];
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&num_points.to_le_bytes());
    for c in &counters {
        bytes.extend_from_slice(&c.to_le_bytes());
    }
    std::fs::write(&data_path, &bytes).unwrap();

    let data = CoverageData::read_binary(&data_path).unwrap();
    assert_eq!(data.counters, counters);
}

#[test]
fn coverage_data_empty() {
    let dir = tempfile::tempdir().unwrap();
    let data_path = dir.path().join("coverage-data.bin");

    let num_points: i64 = 0;
    let bytes = num_points.to_le_bytes().to_vec();
    std::fs::write(&data_path, &bytes).unwrap();

    let data = CoverageData::read_binary(&data_path).unwrap();
    assert!(data.counters.is_empty());
}

#[test]
fn coverage_data_rejects_truncated() {
    let dir = tempfile::tempdir().unwrap();
    let data_path = dir.path().join("coverage-data.bin");

    // Write header saying 5 points but no counter data
    let num_points: i64 = 5;
    let bytes = num_points.to_le_bytes().to_vec();
    std::fs::write(&data_path, &bytes).unwrap();

    assert!(CoverageData::read_binary(&data_path).is_err());
}

// ── Report generation tests ─────────────────────────────────────────────────

#[test]
fn terminal_report_all_covered() {
    let map = CoverageMap {
        points: vec![
            CoveragePoint {
                id: 0, file_id: 0, byte_offset: 0, line: 1, column: 1,
                end_line: 1, end_column: 10,
                kind: CoverageKind::FunctionEntry,
                function_name: "main".to_string(),
                branch_id: 0,
            },
            CoveragePoint {
                id: 1, file_id: 0, byte_offset: 10, line: 2, column: 5,
                end_line: 2, end_column: 15,
                kind: CoverageKind::Statement,
                function_name: "main".to_string(),
                branch_id: 0,
            },
            CoveragePoint {
                id: 2, file_id: 0, byte_offset: 25, line: 3, column: 5,
                end_line: 3, end_column: 15,
                kind: CoverageKind::Statement,
                function_name: "main".to_string(),
                branch_id: 0,
            },
        ],
        files: vec![CoverageFile { id: 0, path: "test.pluto".to_string() }],
    };
    let data = CoverageData { counters: vec![1, 3, 2] };
    let stats = generate_terminal_report(&map, &data);
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].covered_lines, 2);
    assert_eq!(stats[0].total_lines, 2);
    assert_eq!(stats[0].covered_functions, 1);
    assert_eq!(stats[0].total_functions, 1);
}

#[test]
fn terminal_report_partial_coverage() {
    let map = CoverageMap {
        points: vec![
            CoveragePoint {
                id: 0, file_id: 0, byte_offset: 0, line: 1, column: 1,
                end_line: 1, end_column: 10,
                kind: CoverageKind::FunctionEntry,
                function_name: "foo".to_string(),
                branch_id: 0,
            },
            CoveragePoint {
                id: 1, file_id: 0, byte_offset: 10, line: 2, column: 5,
                end_line: 2, end_column: 15,
                kind: CoverageKind::Statement,
                function_name: "foo".to_string(),
                branch_id: 0,
            },
            CoveragePoint {
                id: 2, file_id: 0, byte_offset: 30, line: 5, column: 1,
                end_line: 5, end_column: 10,
                kind: CoverageKind::FunctionEntry,
                function_name: "bar".to_string(),
                branch_id: 0,
            },
            CoveragePoint {
                id: 3, file_id: 0, byte_offset: 40, line: 6, column: 5,
                end_line: 6, end_column: 15,
                kind: CoverageKind::Statement,
                function_name: "bar".to_string(),
                branch_id: 0,
            },
        ],
        files: vec![CoverageFile { id: 0, path: "test.pluto".to_string() }],
    };
    // foo called, bar not
    let data = CoverageData { counters: vec![1, 5, 0, 0] };
    let stats = generate_terminal_report(&map, &data);
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].total_lines, 2);
    assert_eq!(stats[0].covered_lines, 1);
    assert_eq!(stats[0].total_functions, 2);
    assert_eq!(stats[0].covered_functions, 1);
}

#[test]
fn terminal_report_no_points() {
    let map = CoverageMap {
        points: vec![],
        files: vec![CoverageFile { id: 0, path: "empty.pluto".to_string() }],
    };
    let data = CoverageData { counters: vec![] };
    let stats = generate_terminal_report(&map, &data);
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].total_lines, 0);
    assert_eq!(stats[0].covered_lines, 0);
}

// ── Coverage map JSON roundtrip ─────────────────────────────────────────────

#[test]
fn coverage_map_json_roundtrip() {
    let map = build_map("fn main() {\n    let x = 1\n    let y = 2\n}\n");
    let dir = tempfile::tempdir().unwrap();
    let json_path = dir.path().join("coverage-map.json");
    map.write_json(&json_path).unwrap();
    let loaded = CoverageMap::read_json(&json_path).unwrap();
    assert_eq!(loaded.points.len(), map.points.len());
    assert_eq!(loaded.files.len(), map.files.len());
    for (a, b) in loaded.points.iter().zip(map.points.iter()) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.byte_offset, b.byte_offset);
        assert_eq!(a.line, b.line);
        assert_eq!(a.kind, b.kind);
    }
}

// ── Line index tests ────────────────────────────────────────────────────────

#[test]
fn line_index_empty_source() {
    let idx = LineIndex::new("");
    assert_eq!(idx.line_col(0), (1, 1));
}

#[test]
fn line_index_single_char() {
    let idx = LineIndex::new("a");
    assert_eq!(idx.line_col(0), (1, 1));
}

#[test]
fn line_index_trailing_newline() {
    let idx = LineIndex::new("abc\n");
    assert_eq!(idx.line_col(0), (1, 1));
    assert_eq!(idx.line_col(3), (1, 4));  // the \n character
    assert_eq!(idx.line_col(4), (2, 1));  // after the \n
}

// ── End-to-end compilation with coverage ────────────────────────────────────

#[test]
fn coverage_compilation_produces_map() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_path, "fn main() {\n    let x = 1\n    print(x)\n}\n").unwrap();

    let map = plutoc::compile_file_with_coverage(
        &source_path,
        &bin_path,
        None,

    ).unwrap();

    assert!(map.num_points() >= 3, "expected at least 3 points, got {}", map.num_points());
    assert_eq!(map.files.len(), 1);
}

#[test]
fn coverage_end_to_end_run() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_path, r#"
fn add(a: int, b: int) int {
    return a + b
}
fn main() {
    let x = add(1, 2)
    print(x)
}
"#).unwrap();

    let map = plutoc::compile_file_with_coverage(
        &source_path,
        &bin_path,
        None,

    ).unwrap();

    // Write coverage map
    let cov_dir = dir.path().join(".pluto-coverage");
    std::fs::create_dir_all(&cov_dir).unwrap();
    map.write_json(&cov_dir.join("coverage-map.json")).unwrap();

    // Run the binary (it will write coverage-data.bin via atexit)
    let status = std::process::Command::new(&bin_path)
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(status.success(), "binary should exit successfully");

    // Read coverage data
    let data_path = cov_dir.join("coverage-data.bin");
    assert!(data_path.exists(), "coverage data file should exist after run");

    let data = CoverageData::read_binary(&data_path).unwrap();
    assert!(!data.counters.is_empty(), "should have counter data");

    // Generate report
    let stats = generate_terminal_report(&map, &data);
    assert_eq!(stats.len(), 1);
    // All statements in main and add should be covered
    assert!(stats[0].covered_lines > 0, "some lines should be covered");
    // At least 3 of the lines should be covered (add body + main body)
    assert!(stats[0].covered_lines >= 3, "expected at least 3 covered lines, got {}", stats[0].covered_lines);
}

#[test]
fn coverage_uncovered_function() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_path, r#"
fn unused() int {
    return 42
}
fn main() {
    print(1)
}
"#).unwrap();

    let map = plutoc::compile_file_with_coverage(
        &source_path,
        &bin_path,
        None,

    ).unwrap();

    let cov_dir = dir.path().join(".pluto-coverage");
    std::fs::create_dir_all(&cov_dir).unwrap();

    let status = std::process::Command::new(&bin_path)
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(status.success());

    let data = CoverageData::read_binary(&cov_dir.join("coverage-data.bin")).unwrap();
    let stats = generate_terminal_report(&map, &data);

    // Should have partial coverage: main covered, unused not
    assert!(stats[0].covered_lines < stats[0].total_lines,
        "not all lines should be covered (unused function), covered={} total={}",
        stats[0].covered_lines, stats[0].total_lines);
    assert!(stats[0].covered_functions < stats[0].total_functions,
        "not all functions should be covered, covered={} total={}",
        stats[0].covered_functions, stats[0].total_functions);
}

#[test]
fn coverage_branch_partial() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_path, r#"
fn main() {
    let x = 5
    if x > 10 {
        print(1)
    } else {
        print(2)
    }
}
"#).unwrap();

    let map = plutoc::compile_file_with_coverage(
        &source_path,
        &bin_path,
        None,

    ).unwrap();

    let status = std::process::Command::new(&bin_path)
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(status.success());

    let cov_dir = dir.path().join(".pluto-coverage");
    let data = CoverageData::read_binary(&cov_dir.join("coverage-data.bin")).unwrap();
    let stats = generate_terminal_report(&map, &data);

    // Then branch should NOT be covered (x=5, condition false)
    // Else branch should be covered
    assert!(stats[0].covered_lines < stats[0].total_lines,
        "should have partial coverage with only-else branch taken");
}

// ── Phase 2: Branch coverage tests ─────────────────────────────────────────

#[test]
fn coverage_map_has_branch_points_for_if_else() {
    let map = build_map(
        r#"
fn main() {
    let x = 42
    if x > 10 {
        print(1)
    } else {
        print(0)
    }
}
"#,
    );
    let then_points: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::BranchThen).collect();
    let else_points: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::BranchElse).collect();
    assert_eq!(then_points.len(), 1, "should have 1 BranchThen point");
    assert_eq!(else_points.len(), 1, "should have 1 BranchElse point");
    assert_eq!(then_points[0].branch_id, 1);
    assert_eq!(else_points[0].branch_id, 1);
}

#[test]
fn coverage_map_has_implicit_else_branch() {
    let map = build_map(
        r#"
fn main() {
    let x = 42
    if x > 10 {
        print(1)
    }
}
"#,
    );
    let then_points: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::BranchThen).collect();
    let else_points: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::BranchElse).collect();
    assert_eq!(then_points.len(), 1, "should have 1 BranchThen point");
    assert_eq!(else_points.len(), 1, "should have 1 BranchElse for implicit else");
    assert_eq!(else_points[0].branch_id, 2, "implicit else uses branch_id 2");
}

#[test]
fn coverage_map_has_loop_entry_points() {
    let map = build_map(
        r#"
fn main() {
    let mut i = 0
    while i < 10 {
        i = i + 1
    }
}
"#,
    );
    let loop_points: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::LoopEntry).collect();
    assert_eq!(loop_points.len(), 1, "should have 1 LoopEntry point");
    assert_eq!(loop_points[0].branch_id, 1);
}

#[test]
fn coverage_map_has_for_loop_entry() {
    let map = build_map(
        r#"
fn main() {
    for i in 0..5 {
        print(i)
    }
}
"#,
    );
    let loop_points: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::LoopEntry).collect();
    assert_eq!(loop_points.len(), 1, "should have 1 LoopEntry for for-loop");
}

#[test]
fn coverage_map_has_match_arm_points() {
    let map = build_map(
        "enum Color {\n    Red\n    Green\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Color.Red {\n            print(1)\n        }\n        Color.Green {\n            print(2)\n        }\n        Color.Blue {\n            print(3)\n        }\n    }\n}\n",
    );
    let arm_points: Vec<_> = map.points.iter().filter(|p| matches!(p.kind, CoverageKind::MatchArm { .. })).collect();
    assert_eq!(arm_points.len(), 3, "should have 3 MatchArm points");
    for point in &arm_points {
        assert_eq!(point.branch_id, 1);
    }
}

#[test]
fn coverage_branch_count_in_report() {
    let map = CoverageMap {
        points: vec![
            CoveragePoint {
                id: 0, file_id: 0, byte_offset: 0, line: 1, column: 1,
                end_line: 1, end_column: 10,
                kind: CoverageKind::Statement,
                function_name: "main".to_string(),
                branch_id: 0,
            },
            CoveragePoint {
                id: 1, file_id: 0, byte_offset: 20, line: 3, column: 5,
                end_line: 3, end_column: 15,
                kind: CoverageKind::BranchThen,
                function_name: "main".to_string(),
                branch_id: 1,
            },
            CoveragePoint {
                id: 2, file_id: 0, byte_offset: 40, line: 5, column: 5,
                end_line: 5, end_column: 15,
                kind: CoverageKind::BranchElse,
                function_name: "main".to_string(),
                branch_id: 1,
            },
        ],
        files: vec![CoverageFile { id: 0, path: "test.pluto".to_string() }],
    };
    // Statement covered, then branch covered, else branch not covered
    let data = CoverageData { counters: vec![1, 3, 0] };
    let stats = generate_terminal_report(&map, &data);
    assert_eq!(stats[0].total_branches, 2);
    assert_eq!(stats[0].covered_branches, 1);
}

#[test]
fn coverage_if_then_branch_hit() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_path, r#"
fn main() {
    let x = 20
    if x > 10 {
        print(1)
    } else {
        print(0)
    }
}
"#).unwrap();

    let map = plutoc::compile_file_with_coverage(
        &source_path, &bin_path, None,
    ).unwrap();

    let status = std::process::Command::new(&bin_path)
        .current_dir(dir.path())
        .status().unwrap();
    assert!(status.success());

    let cov_dir = dir.path().join(".pluto-coverage");
    let data = CoverageData::read_binary(&cov_dir.join("coverage-data.bin")).unwrap();

    // Find the BranchThen and BranchElse points
    let then_pts: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::BranchThen).collect();
    let else_pts: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::BranchElse).collect();
    assert!(!then_pts.is_empty(), "should have BranchThen points");
    assert!(!else_pts.is_empty(), "should have BranchElse points");

    // x=20 > 10, so then branch should be hit, else should not
    assert!(data.counters[then_pts[0].id as usize] > 0,
        "then branch should be hit (x=20 > 10)");
    assert_eq!(data.counters[else_pts[0].id as usize], 0,
        "else branch should NOT be hit");
}

#[test]
fn coverage_if_else_branch_hit() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_path, r#"
fn main() {
    let x = 5
    if x > 10 {
        print(1)
    } else {
        print(0)
    }
}
"#).unwrap();

    let map = plutoc::compile_file_with_coverage(
        &source_path, &bin_path, None,
    ).unwrap();

    let status = std::process::Command::new(&bin_path)
        .current_dir(dir.path())
        .status().unwrap();
    assert!(status.success());

    let cov_dir = dir.path().join(".pluto-coverage");
    let data = CoverageData::read_binary(&cov_dir.join("coverage-data.bin")).unwrap();

    let then_pts: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::BranchThen).collect();
    let else_pts: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::BranchElse).collect();

    // x=5, not > 10, so else branch hit, then not
    assert_eq!(data.counters[then_pts[0].id as usize], 0,
        "then branch should NOT be hit (x=5)");
    assert!(data.counters[else_pts[0].id as usize] > 0,
        "else branch should be hit");
}

#[test]
fn coverage_implicit_else_branch_hit() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_path, r#"
fn main() {
    let x = 5
    if x > 10 {
        print(1)
    }
    print(0)
}
"#).unwrap();

    let map = plutoc::compile_file_with_coverage(
        &source_path, &bin_path, None,
    ).unwrap();

    let status = std::process::Command::new(&bin_path)
        .current_dir(dir.path())
        .status().unwrap();
    assert!(status.success());

    let cov_dir = dir.path().join(".pluto-coverage");
    let data = CoverageData::read_binary(&cov_dir.join("coverage-data.bin")).unwrap();

    let then_pts: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::BranchThen).collect();
    let else_pts: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::BranchElse).collect();

    // x=5, condition false, so implicit else path taken
    assert_eq!(data.counters[then_pts[0].id as usize], 0,
        "then branch should NOT be hit");
    assert!(data.counters[else_pts[0].id as usize] > 0,
        "implicit else branch should be hit");
}

#[test]
fn coverage_loop_body_hit() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_path, r#"
fn main() {
    let mut i = 0
    while i < 3 {
        i = i + 1
    }
}
"#).unwrap();

    let map = plutoc::compile_file_with_coverage(
        &source_path, &bin_path, None,
    ).unwrap();

    let status = std::process::Command::new(&bin_path)
        .current_dir(dir.path())
        .status().unwrap();
    assert!(status.success());

    let cov_dir = dir.path().join(".pluto-coverage");
    let data = CoverageData::read_binary(&cov_dir.join("coverage-data.bin")).unwrap();

    let loop_pts: Vec<_> = map.points.iter().filter(|p| p.kind == CoverageKind::LoopEntry).collect();
    assert!(!loop_pts.is_empty(), "should have LoopEntry points");

    // Loop runs 3 times
    assert!(data.counters[loop_pts[0].id as usize] >= 3,
        "loop body should be hit at least 3 times, got {}",
        data.counters[loop_pts[0].id as usize]);
}

#[test]
fn coverage_match_arm_hit() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_path,
        "enum Color {\n    Red\n    Green\n    Blue\n}\n\nfn main() {\n    let c = Color.Green\n    match c {\n        Color.Red {\n            print(1)\n        }\n        Color.Green {\n            print(2)\n        }\n        Color.Blue {\n            print(3)\n        }\n    }\n}\n"
    ).unwrap();

    let map = plutoc::compile_file_with_coverage(
        &source_path, &bin_path, None,
    ).unwrap();

    let status = std::process::Command::new(&bin_path)
        .current_dir(dir.path())
        .status().unwrap();
    assert!(status.success());

    let cov_dir = dir.path().join(".pluto-coverage");
    let data = CoverageData::read_binary(&cov_dir.join("coverage-data.bin")).unwrap();

    let arm_pts: Vec<_> = map.points.iter()
        .filter(|p| matches!(p.kind, CoverageKind::MatchArm { .. }))
        .collect();
    assert_eq!(arm_pts.len(), 3, "should have 3 match arm points");

    // Only the Green arm should be hit (index 1)
    let mut hit_count = 0;
    for pt in &arm_pts {
        if data.counters[pt.id as usize] > 0 {
            hit_count += 1;
        }
    }
    assert_eq!(hit_count, 1, "exactly one match arm should be hit");
}

#[test]
fn coverage_branch_report_stats() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_path, r#"
fn main() {
    let x = 5
    if x > 10 {
        print(1)
    } else {
        print(0)
    }
}
"#).unwrap();

    let map = plutoc::compile_file_with_coverage(
        &source_path, &bin_path, None,
    ).unwrap();

    let status = std::process::Command::new(&bin_path)
        .current_dir(dir.path())
        .status().unwrap();
    assert!(status.success());

    let cov_dir = dir.path().join(".pluto-coverage");
    let data = CoverageData::read_binary(&cov_dir.join("coverage-data.bin")).unwrap();
    let stats = generate_terminal_report(&map, &data);

    // Should have 2 branch points (then + else) and only 1 covered
    assert_eq!(stats[0].total_branches, 2, "should have 2 branches");
    assert_eq!(stats[0].covered_branches, 1, "should have 1 covered branch (else)");
}

// ── Phase 3: LCOV + JSON output tests ──────────────────────────────────────

#[test]
fn coverage_lcov_basic_format() {
    use plutoc::coverage::*;

    let map = CoverageMap {
        points: vec![
            CoveragePoint {
                id: 0, file_id: 0, byte_offset: 0, line: 1, column: 1,
                end_line: 1, end_column: 10,
                kind: CoverageKind::FunctionEntry,
                function_name: "main".to_string(),
                branch_id: 0,
            },
            CoveragePoint {
                id: 1, file_id: 0, byte_offset: 10, line: 2, column: 5,
                end_line: 2, end_column: 15,
                kind: CoverageKind::Statement,
                function_name: "main".to_string(),
                branch_id: 0,
            },
            CoveragePoint {
                id: 2, file_id: 0, byte_offset: 25, line: 3, column: 5,
                end_line: 3, end_column: 15,
                kind: CoverageKind::Statement,
                function_name: "main".to_string(),
                branch_id: 0,
            },
        ],
        files: vec![CoverageFile { id: 0, path: "test.pluto".to_string() }],
    };
    let data = CoverageData { counters: vec![1, 3, 0] };
    let lcov = generate_lcov(&map, &data);

    assert!(lcov.contains("TN:"), "should have test name record");
    assert!(lcov.contains("SF:test.pluto"), "should have source file");
    assert!(lcov.contains("FN:1,main"), "should have function record");
    assert!(lcov.contains("FNDA:1,main"), "should have function hit count");
    assert!(lcov.contains("FNF:1"), "should have function found count");
    assert!(lcov.contains("FNH:1"), "should have function hit count");
    assert!(lcov.contains("DA:2,3"), "line 2 hit 3 times");
    assert!(lcov.contains("DA:3,0"), "line 3 not hit");
    assert!(lcov.contains("LF:2"), "2 lines found");
    assert!(lcov.contains("LH:1"), "1 line hit");
    assert!(lcov.contains("end_of_record"), "should have end record");
}

#[test]
fn coverage_lcov_with_branches() {
    use plutoc::coverage::*;

    let map = CoverageMap {
        points: vec![
            CoveragePoint {
                id: 0, file_id: 0, byte_offset: 0, line: 1, column: 1,
                end_line: 1, end_column: 10,
                kind: CoverageKind::Statement,
                function_name: "main".to_string(),
                branch_id: 0,
            },
            CoveragePoint {
                id: 1, file_id: 0, byte_offset: 20, line: 3, column: 5,
                end_line: 3, end_column: 15,
                kind: CoverageKind::BranchThen,
                function_name: "main".to_string(),
                branch_id: 1,
            },
            CoveragePoint {
                id: 2, file_id: 0, byte_offset: 40, line: 5, column: 5,
                end_line: 5, end_column: 15,
                kind: CoverageKind::BranchElse,
                function_name: "main".to_string(),
                branch_id: 1,
            },
        ],
        files: vec![CoverageFile { id: 0, path: "test.pluto".to_string() }],
    };
    let data = CoverageData { counters: vec![1, 2, 0] };
    let lcov = generate_lcov(&map, &data);

    // Should have BRDA records for branches
    assert!(lcov.contains("BRDA:"), "should have branch records");
    assert!(lcov.contains("BRF:2"), "2 branches found");
    assert!(lcov.contains("BRH:1"), "1 branch hit");
}

#[test]
fn coverage_json_basic_format() {
    use plutoc::coverage::*;

    let map = CoverageMap {
        points: vec![
            CoveragePoint {
                id: 0, file_id: 0, byte_offset: 0, line: 1, column: 1,
                end_line: 1, end_column: 10,
                kind: CoverageKind::FunctionEntry,
                function_name: "add".to_string(),
                branch_id: 0,
            },
            CoveragePoint {
                id: 1, file_id: 0, byte_offset: 10, line: 2, column: 5,
                end_line: 2, end_column: 15,
                kind: CoverageKind::Statement,
                function_name: "add".to_string(),
                branch_id: 0,
            },
        ],
        files: vec![CoverageFile { id: 0, path: "math.pluto".to_string() }],
    };
    let data = CoverageData { counters: vec![5, 5] };
    let report = generate_json_report(&map, &data);

    assert_eq!(report.summary.total_lines, 1);
    assert_eq!(report.summary.covered_lines, 1);
    assert_eq!(report.summary.total_functions, 1);
    assert_eq!(report.summary.covered_functions, 1);
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].path, "math.pluto");
    assert_eq!(report.files[0].function_details.len(), 1);
    assert_eq!(report.files[0].function_details[0].name, "add");
    assert_eq!(report.files[0].function_details[0].hit_count, 5);
}

#[test]
fn coverage_json_serializes() {
    use plutoc::coverage::*;

    let map = CoverageMap {
        points: vec![
            CoveragePoint {
                id: 0, file_id: 0, byte_offset: 0, line: 1, column: 1,
                end_line: 1, end_column: 10,
                kind: CoverageKind::Statement,
                function_name: "main".to_string(),
                branch_id: 0,
            },
        ],
        files: vec![CoverageFile { id: 0, path: "test.pluto".to_string() }],
    };
    let data = CoverageData { counters: vec![1] };
    let report = generate_json_report(&map, &data);

    let json = serde_json::to_string_pretty(&report).unwrap();
    assert!(json.contains("\"total_lines\""), "should have total_lines");
    assert!(json.contains("\"covered_lines\""), "should have covered_lines");
    assert!(json.contains("\"line_percent\""), "should have line_percent");
    assert!(json.contains("\"test.pluto\""), "should have file path");

    // Verify it round-trips
    let parsed: JsonCoverageReport = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.summary.total_lines, 1);
}

#[test]
fn coverage_lcov_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_path, r#"
fn add(a: int, b: int) int {
    return a + b
}
fn main() {
    let x = add(1, 2)
    if x > 0 {
        print(x)
    } else {
        print(0)
    }
}
"#).unwrap();

    let map = plutoc::compile_file_with_coverage(
        &source_path, &bin_path, None,
    ).unwrap();

    let status = std::process::Command::new(&bin_path)
        .current_dir(dir.path())
        .status().unwrap();
    assert!(status.success());

    let cov_dir = dir.path().join(".pluto-coverage");
    let data = CoverageData::read_binary(&cov_dir.join("coverage-data.bin")).unwrap();

    let lcov = plutoc::coverage::generate_lcov(&map, &data);

    // Verify key LCOV sections
    assert!(lcov.contains("SF:"), "LCOV should have source file");
    assert!(lcov.contains("FN:"), "LCOV should have function records");
    assert!(lcov.contains("DA:"), "LCOV should have line records");
    assert!(lcov.contains("BRDA:"), "LCOV should have branch records");
    assert!(lcov.contains("end_of_record"), "LCOV should end properly");

    // Verify functions are listed
    assert!(lcov.contains("main"), "LCOV should contain main function");
    assert!(lcov.contains("add"), "LCOV should contain add function");
}

#[test]
fn coverage_json_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_path, r#"
fn unused() int {
    return 42
}
fn main() {
    print(1)
}
"#).unwrap();

    let map = plutoc::compile_file_with_coverage(
        &source_path, &bin_path, None,
    ).unwrap();

    let status = std::process::Command::new(&bin_path)
        .current_dir(dir.path())
        .status().unwrap();
    assert!(status.success());

    let cov_dir = dir.path().join(".pluto-coverage");
    let data = CoverageData::read_binary(&cov_dir.join("coverage-data.bin")).unwrap();

    let report = plutoc::coverage::generate_json_report(&map, &data);

    // Should have partial coverage
    assert!(report.summary.covered_functions < report.summary.total_functions,
        "not all functions should be covered");
    assert!(report.summary.line_percent < 100.0,
        "should not be 100% line coverage");

    // Verify JSON serializes cleanly
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.is_empty());
}

// ── Phase 4: HTML report tests ─────────────────────────────────────────────

#[test]
fn coverage_html_contains_template_structure() {
    use plutoc::coverage::*;

    let dir = tempfile::tempdir().unwrap();
    // Write a dummy source file so generate_html_report can read it
    std::fs::write(dir.path().join("test.pluto"), "fn main() {\n    print(1)\n}\n").unwrap();

    let map = CoverageMap {
        points: vec![
            CoveragePoint {
                id: 0, file_id: 0, byte_offset: 0, line: 1, column: 1,
                end_line: 1, end_column: 10,
                kind: CoverageKind::FunctionEntry,
                function_name: "main".to_string(),
                branch_id: 0,
            },
            CoveragePoint {
                id: 1, file_id: 0, byte_offset: 12, line: 2, column: 5,
                end_line: 2, end_column: 15,
                kind: CoverageKind::Statement,
                function_name: "main".to_string(),
                branch_id: 0,
            },
        ],
        files: vec![CoverageFile { id: 0, path: "test.pluto".to_string() }],
    };
    let data = CoverageData { counters: vec![1, 3] };

    let html = generate_html_report(&map, &data, dir.path());

    // Should contain key HTML structure
    assert!(html.contains("<!DOCTYPE html>"), "should be valid HTML");
    assert!(html.contains("Pluto Coverage Report"), "should have title");
    assert!(html.contains("treemap"), "should have treemap element");
    assert!(html.contains("source-view"), "should have source view");
    assert!(html.contains("file-list"), "should have file list");
    assert!(html.contains("func-table"), "should have function table");
}

#[test]
fn coverage_html_embeds_coverage_data() {
    use plutoc::coverage::*;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("test.pluto"), "fn main() {\n    let x = 1\n}\n").unwrap();

    let map = CoverageMap {
        points: vec![
            CoveragePoint {
                id: 0, file_id: 0, byte_offset: 0, line: 1, column: 1,
                end_line: 1, end_column: 10,
                kind: CoverageKind::FunctionEntry,
                function_name: "main".to_string(),
                branch_id: 0,
            },
            CoveragePoint {
                id: 1, file_id: 0, byte_offset: 12, line: 2, column: 5,
                end_line: 2, end_column: 15,
                kind: CoverageKind::Statement,
                function_name: "main".to_string(),
                branch_id: 0,
            },
        ],
        files: vec![CoverageFile { id: 0, path: "test.pluto".to_string() }],
    };
    let data = CoverageData { counters: vec![1, 1] };

    let html = generate_html_report(&map, &data, dir.path());

    // Should NOT contain the placeholder anymore
    assert!(!html.contains("/*COVERAGE_DATA*/null"), "placeholder should be replaced");

    // Should contain coverage data JSON inline
    assert!(html.contains("\"total_lines\""), "should embed line stats");
    assert!(html.contains("\"covered_lines\""), "should embed covered count");
    assert!(html.contains("\"line_percent\""), "should embed percentage");
    assert!(html.contains("test.pluto"), "should reference source file");
    assert!(html.contains("main"), "should reference function name");
}

#[test]
fn coverage_html_embeds_source_code() {
    use plutoc::coverage::*;

    let dir = tempfile::tempdir().unwrap();
    let source_content = "fn hello() {\n    print(\"world\")\n}\n";
    std::fs::write(dir.path().join("hello.pluto"), source_content).unwrap();

    let map = CoverageMap {
        points: vec![CoveragePoint {
            id: 0, file_id: 0, byte_offset: 0, line: 1, column: 1,
            end_line: 1, end_column: 10,
            kind: CoverageKind::FunctionEntry,
            function_name: "hello".to_string(),
            branch_id: 0,
        }],
        files: vec![CoverageFile { id: 0, path: "hello.pluto".to_string() }],
    };
    let data = CoverageData { counters: vec![1] };

    let html = generate_html_report(&map, &data, dir.path());

    // Source code should be embedded in the sources map
    assert!(html.contains("fn hello()"), "should embed source code");
    assert!(html.contains("sources"), "should have sources field");
}

#[test]
fn coverage_html_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_path, r#"
fn add(a: int, b: int) int {
    return a + b
}
fn main() {
    let x = add(1, 2)
    print(x)
}
"#).unwrap();

    let map = plutoc::compile_file_with_coverage(
        &source_path, &bin_path, None,
    ).unwrap();

    let status = std::process::Command::new(&bin_path)
        .current_dir(dir.path())
        .status().unwrap();
    assert!(status.success());

    let cov_dir = dir.path().join(".pluto-coverage");
    let data = CoverageData::read_binary(&cov_dir.join("coverage-data.bin")).unwrap();

    let html = plutoc::coverage::generate_html_report(&map, &data, dir.path());

    // Verify it's a complete, valid HTML page with embedded data
    assert!(html.contains("<!DOCTYPE html>"), "should be valid HTML");
    assert!(!html.contains("/*COVERAGE_DATA*/null"), "placeholder should be replaced");
    assert!(html.contains("\"line_percent\""), "should have coverage stats");
    assert!(html.contains("main"), "should reference main function");
    assert!(html.contains("add"), "should reference add function");

    // Write it out and verify the file is reasonable size
    let report_path = dir.path().join("report.html");
    std::fs::write(&report_path, &html).unwrap();
    let file_size = std::fs::metadata(&report_path).unwrap().len();
    assert!(file_size > 1000, "HTML report should be >1KB, got {} bytes", file_size);
}

#[test]
fn coverage_html_missing_source_still_works() {
    use plutoc::coverage::*;

    let dir = tempfile::tempdir().unwrap();
    // Don't create the source file — the report should still generate

    let map = CoverageMap {
        points: vec![CoveragePoint {
            id: 0, file_id: 0, byte_offset: 0, line: 1, column: 1,
            end_line: 1, end_column: 10,
            kind: CoverageKind::Statement,
            function_name: "main".to_string(),
            branch_id: 0,
        }],
        files: vec![CoverageFile { id: 0, path: "missing.pluto".to_string() }],
    };
    let data = CoverageData { counters: vec![1] };

    let html = generate_html_report(&map, &data, dir.path());

    // Should still produce valid HTML even without source
    assert!(html.contains("<!DOCTYPE html>"), "should be valid HTML");
    assert!(!html.contains("/*COVERAGE_DATA*/null"), "placeholder should be replaced");
}

// ── Phase 5: file_id + source_len filtering edge cases ──────────────────────

/// Helper: parse source and manually set file_id on specific functions.
/// Functions with file_id=1 won't have a SourceMap entry, so they'll be excluded.
fn build_map_with_foreign_functions(source: &str, foreign_fn_indices: &[usize]) -> CoverageMap {
    let mut program = plutoc::parse_source(source).unwrap();
    for &idx in foreign_fn_indices {
        if idx < program.functions.len() {
            program.functions[idx].span.file_id = 1; // imported module (no SourceMap entry)
        }
    }
    let sm = make_source_map(source); // Only file_id=0 has a SourceMap entry
    build_coverage_map(&program, &sm)
}

#[test]
fn coverage_excludes_functions_with_foreign_file_id() {
    let source = r#"
fn helper() int {
    return 42
}
fn main() {
    let x = helper()
    print(x)
}
"#;
    // Mark helper() as foreign (file_id=1)
    let map = build_map_with_foreign_functions(source, &[0]);

    // Only main should appear in coverage, not helper
    let entries: Vec<_> = map.points.iter()
        .filter(|p| p.kind == CoverageKind::FunctionEntry)
        .collect();
    assert_eq!(entries.len(), 1, "only one function entry (main), got {}", entries.len());
    assert_eq!(entries[0].function_name, "main");

    // No coverage points from helper's body
    for point in &map.points {
        assert_ne!(point.function_name, "helper",
            "no coverage points should come from foreign function");
    }
}

#[test]
fn coverage_excludes_synthetic_file_id() {
    use plutoc::span::SYNTHETIC_FILE_ID;

    let source = r#"
fn real_fn() int {
    return 1
}
fn main() {
    print(real_fn())
}
"#;
    let mut program = plutoc::parse_source(source).unwrap();
    // Mark real_fn as synthetic (reflection-generated, no SourceMap entry)
    program.functions[0].span.file_id = SYNTHETIC_FILE_ID;

    let sm = make_source_map(source);
    let map = build_coverage_map(&program, &sm);

    let entries: Vec<_> = map.points.iter()
        .filter(|p| p.kind == CoverageKind::FunctionEntry)
        .collect();
    assert_eq!(entries.len(), 1, "only main should have entry point");
    assert_eq!(entries[0].function_name, "main");
}

#[test]
fn coverage_excludes_monomorphized_function_spans() {
    // Simulate a monomorphized copy: function with body stmts beyond source length
    let source = "fn main() {\n    let x = 1\n}\n";
    let mut program = plutoc::parse_source(source).unwrap();

    // Add a "monomorphized copy" — clone main and offset its spans
    let mut mono_copy = program.functions[0].clone();
    mono_copy.node.name.node = "identity__int".to_string();
    let offset = source.len() + 10_000_000; // way beyond source
    mono_copy.node.name.span.start = offset;
    mono_copy.node.name.span.end = offset + 10;
    for stmt in &mut mono_copy.node.body.node.stmts {
        stmt.span.start = offset;
        stmt.span.end = offset + 10;
    }
    program.functions.push(mono_copy);

    let sm = make_source_map(source);
    let map = build_coverage_map(&program, &sm);

    // Only main should appear — the monomorphized copy should be skipped
    let entries: Vec<_> = map.points.iter()
        .filter(|p| p.kind == CoverageKind::FunctionEntry)
        .collect();
    assert_eq!(entries.len(), 1, "only original function, not mono copy");
    assert_eq!(entries[0].function_name, "main");
}

#[test]
fn coverage_excludes_class_methods_with_foreign_file_id() {
    let source = r#"
class Local {
    value: int

    fn get(self) int {
        return self.value
    }
}
fn main() {
    let c = Local { value: 42 }
    print(c.get())
}
"#;
    let mut program = plutoc::parse_source(source).unwrap();
    // Mark the class as foreign
    assert!(!program.classes.is_empty(), "should have a class");
    program.classes[0].span.file_id = 2; // No SourceMap entry for file_id=2

    let sm = make_source_map(source);
    let map = build_coverage_map(&program, &sm);

    // Class methods should not appear in coverage
    let entries: Vec<_> = map.points.iter()
        .filter(|p| p.kind == CoverageKind::FunctionEntry)
        .collect();
    assert_eq!(entries.len(), 1, "only main, not class methods");
    assert_eq!(entries[0].function_name, "main");

    for point in &map.points {
        assert!(!point.function_name.contains("Local."),
            "no coverage points from foreign class methods, found: {}", point.function_name);
    }
}

#[test]
fn coverage_mixed_file_ids_only_entry_file() {
    let source = r#"
fn local_a() int {
    return 1
}
fn local_b() int {
    return 2
}
fn main() {
    print(local_a())
    print(local_b())
}
"#;
    let mut program = plutoc::parse_source(source).unwrap();
    // Mark local_b as foreign (file_id=1, no SourceMap entry), keep local_a and main as entry (file_id=0)
    program.functions[1].span.file_id = 1;

    let sm = make_source_map(source);
    let map = build_coverage_map(&program, &sm);

    let entries: Vec<_> = map.points.iter()
        .filter(|p| p.kind == CoverageKind::FunctionEntry)
        .collect();
    let names: Vec<_> = entries.iter().map(|e| e.function_name.as_str()).collect();
    assert!(names.contains(&"local_a"), "local_a should be included");
    assert!(names.contains(&"main"), "main should be included");
    assert!(!names.contains(&"local_b"), "local_b (foreign) should be excluded");
}

#[test]
fn coverage_empty_function_body_no_crash() {
    // An empty function body should not panic in scan_function_with_name
    // (the first-stmt check guards against empty stmts vec)
    let source = "fn empty() {\n}\nfn main() {\n}\n";
    let map = build_map(source);

    // Both functions have entry points but no statement points
    let entries: Vec<_> = map.points.iter()
        .filter(|p| p.kind == CoverageKind::FunctionEntry)
        .collect();
    assert_eq!(entries.len(), 2, "both empty and main should have entries");
}

#[test]
fn coverage_generics_no_duplicate_points() {
    // End-to-end: generics produce monomorphized copies with offset spans.
    // Coverage should only report the original source lines.
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&source_path, r#"
fn identity<T>(x: T) T {
    return x
}
fn main() {
    let a = identity(42)
    let b = identity("hello")
    print(a)
    print(b)
}
"#).unwrap();

    let map = plutoc::compile_file_with_coverage(
        &source_path, &bin_path, None,
    ).unwrap();

    // The generic function `identity` should appear once as a function entry,
    // NOT once per monomorphized copy (identity__int, identity__string)
    let identity_entries: Vec<_> = map.points.iter()
        .filter(|p| p.kind == CoverageKind::FunctionEntry && p.function_name == "identity")
        .collect();
    // identity may or may not appear (monomorphize replaces the template),
    // but there should be no duplicate coverage points on the same line
    let mut line_function_pairs: Vec<(u32, &str)> = map.points.iter()
        .filter(|p| p.kind == CoverageKind::FunctionEntry)
        .map(|p| (p.line, p.function_name.as_str()))
        .collect();
    line_function_pairs.sort();
    line_function_pairs.dedup();
    assert_eq!(
        line_function_pairs.len(),
        map.points.iter().filter(|p| p.kind == CoverageKind::FunctionEntry).count(),
        "no duplicate function entry points on the same line: {:?}",
        line_function_pairs,
    );

    // All coverage point lines should be within the source file's line range
    let source = std::fs::read_to_string(&source_path).unwrap();
    let max_line = source.lines().count() as u32;
    for point in &map.points {
        assert!(point.line <= max_line + 1,
            "coverage point line {} exceeds source lines {} (function: {}, kind: {:?})",
            point.line, max_line, point.function_name, point.kind);
    }
}

#[test]
fn coverage_source_len_boundary_exact() {
    // Test that a span at exactly source.len() is excluded (it's an invalid offset)
    let source = "fn main() {\n    let x = 1\n}\n";
    let mut program = plutoc::parse_source(source).unwrap();

    // Add a function with first stmt at exactly source.len()
    let mut boundary_fn = program.functions[0].clone();
    boundary_fn.node.name.node = "boundary".to_string();
    let boundary = source.len();
    boundary_fn.node.name.span.start = boundary;
    boundary_fn.node.name.span.end = boundary + 5;
    for stmt in &mut boundary_fn.node.body.node.stmts {
        stmt.span.start = boundary;
        stmt.span.end = boundary + 5;
    }
    program.functions.push(boundary_fn);

    let sm = make_source_map(source);
    let map = build_coverage_map(&program, &sm);

    // boundary fn should be excluded (first stmt at source.len() >= source_len)
    for point in &map.points {
        assert_ne!(point.function_name, "boundary",
            "function with spans at source_len boundary should be excluded");
    }
}

#[test]
fn coverage_branch_spans_beyond_source_excluded() {
    // If a branch span (if/else/loop body) is beyond source_len, it should not add branch points
    let source = "fn main() {\n    let x = 1\n    if true {\n        print(x)\n    }\n}\n";
    let mut program = plutoc::parse_source(source).unwrap();

    // Clone main and create a "monomorphized" version with offset branch spans
    let mut mono = program.functions[0].clone();
    mono.node.name.node = "mono_main".to_string();
    let offset = source.len() + 1000;
    mono.node.name.span.start = offset;
    mono.node.name.span.end = offset + 10;
    for stmt in &mut mono.node.body.node.stmts {
        stmt.span.start = offset;
        stmt.span.end = offset + 10;
    }
    mono.node.body.span.start = offset;
    mono.node.body.span.end = offset + 50;
    program.functions.push(mono);

    let sm = make_source_map(source);
    let map = build_coverage_map(&program, &sm);

    // No points from mono_main should appear
    for point in &map.points {
        assert_ne!(point.function_name, "mono_main",
            "monomorphized function should have no coverage points");
    }
}
