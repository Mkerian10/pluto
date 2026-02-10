use lsp_types::Position;

/// Maps byte offsets ↔ LSP line:column positions for a single source file.
pub struct LineIndex {
    /// Byte offset of the start of each line. line_starts[0] == 0 always.
    line_starts: Vec<u32>,
}

impl LineIndex {
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0u32];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self { line_starts }
    }

    /// Convert a byte offset to an LSP Position (0-based line, 0-based UTF-16 column).
    /// For v0.1 we assume ASCII/UTF-8 where each byte = one UTF-16 code unit.
    pub fn offset_to_position(&self, offset: usize) -> Position {
        let offset = offset as u32;
        // Binary search: find the last line_start <= offset
        let line = match self.line_starts.binary_search(&offset) {
            Ok(exact) => exact,
            Err(ins) => ins.saturating_sub(1),
        };
        let col = offset.saturating_sub(self.line_starts[line]);
        Position {
            line: line as u32,
            character: col,
        }
    }

    /// Convert an LSP Position to a byte offset.
    pub fn position_to_offset(&self, pos: Position) -> usize {
        let line = pos.line as usize;
        if line < self.line_starts.len() {
            (self.line_starts[line] + pos.character) as usize
        } else {
            // Past end of file — return last byte
            self.line_starts.last().copied().unwrap_or(0) as usize
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line() {
        let idx = LineIndex::new("hello");
        assert_eq!(idx.offset_to_position(0), Position { line: 0, character: 0 });
        assert_eq!(idx.offset_to_position(3), Position { line: 0, character: 3 });
    }

    #[test]
    fn multi_line() {
        let idx = LineIndex::new("ab\ncd\nef");
        // line 0: "ab\n" starts at 0
        // line 1: "cd\n" starts at 3
        // line 2: "ef"   starts at 6
        assert_eq!(idx.offset_to_position(0), Position { line: 0, character: 0 });
        assert_eq!(idx.offset_to_position(1), Position { line: 0, character: 1 });
        assert_eq!(idx.offset_to_position(3), Position { line: 1, character: 0 });
        assert_eq!(idx.offset_to_position(4), Position { line: 1, character: 1 });
        assert_eq!(idx.offset_to_position(6), Position { line: 2, character: 0 });
        assert_eq!(idx.offset_to_position(7), Position { line: 2, character: 1 });
    }

    #[test]
    fn position_to_offset_roundtrip() {
        let src = "fn main()\n  let x = 1\n  return x\n";
        let idx = LineIndex::new(src);
        for offset in 0..src.len() {
            let pos = idx.offset_to_position(offset);
            let back = idx.position_to_offset(pos);
            assert_eq!(back, offset, "roundtrip failed for offset {offset}");
        }
    }

    #[test]
    fn empty_source() {
        let idx = LineIndex::new("");
        assert_eq!(idx.offset_to_position(0), Position { line: 0, character: 0 });
    }
}
