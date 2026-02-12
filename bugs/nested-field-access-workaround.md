# Nested Field Access Bug - Workaround Applied to Meridian

## Bug Summary
The Pluto parser incorrectly interprets nested field access (e.g., `self.registry.gauges`) as qualified enum variant access (e.g., `module.Enum.Variant`), causing compile-time type errors.

**Full details:** See `bugs/nested-field-access.md`

## Workaround Strategy
Use intermediate variables to break up nested field access into single-level accesses.

## Changes Applied to Meridian

### File: `/Users/matthewkerian/Documents/pluto-projects/meridian/src/collector.pluto`

#### Before (Broken):
```pluto
fn inc_by(mut self, delta: int) {
    let snap = CounterSnapshot { /* ... */ }
    self.registry.merge_counter(snap)  // ERROR: unknown enum 'self.registry'
}
```

#### After (Working):
```pluto
fn inc_by(mut self, delta: int) {
    let snap = CounterSnapshot { /* ... */ }
    let mut reg = self.registry        // Extract to intermediate variable
    reg.merge_counter(snap)             // Single-level field access
}
```

### Affected Methods
1. **Counter.inc_by()** - Line 43-52
   - Changed: `self.registry.merge_counter(snap)` → `let mut reg = self.registry; reg.merge_counter(snap)`

2. **Gauge.set()** - Line 59-68
   - Changed: `self.registry.merge_gauge(snap)` → `let mut reg = self.registry; reg.merge_gauge(snap)`

3. **Gauge.get_current_value()** - Line 89-98
   - Changed: `self.registry.gauges.contains(key)` → `let reg = self.registry; let gauges = reg.gauges; gauges.contains(key)`
   - Changed: `self.registry.gauges[key]` → `gauges[key]`

4. **Histogram.observe()** - Line 105-118
   - Changed: `self.registry.merge_histogram(snap)` → `let mut reg = self.registry; reg.merge_histogram(snap)`

### Key Points
- **Mutability**: Intermediate variables must be `mut` if the method being called requires `mut self`
- **Non-mutating access**: Can use `let reg = self.registry` for read-only methods
- **Deep nesting**: Multiple intermediate variables needed for access like `self.registry.gauges.contains()`

## Test Results
All 11 meridian collector tests passing:
```
✓ counter increments correctly
✓ counter inc_by adds delta
✓ gauge sets value
✓ gauge updates value
✓ gauge inc and dec
✓ gauge inc_by and dec_by
✓ histogram observe adds values
✓ histogram timer measures duration
✓ multiple metrics with different labels
✓ counter and gauge with same name different types
✓ histogram merges observations
```

## Impact Assessment
- **Code bloat**: Minor (~2 extra lines per method with nested access)
- **Performance**: No impact (variables are stack-allocated, likely optimized away)
- **Readability**: Slightly reduced (extra boilerplate), but necessary for now
- **Maintenance**: Temporary - should be removed once parser bug is fixed

## Future Work
This workaround should be reverted once the parser bug is fixed. Grep for "workaround for nested field access bug" or track via the main bug report.

## Testing Command
```bash
cd /Users/matthewkerian/Documents/pluto-projects/meridian
cargo run --manifest-path /Users/matthewkerian/Documents/pluto/Cargo.toml -- \
  test tests/collector/collector_test.pluto \
  --stdlib /Users/matthewkerian/Documents/pluto/stdlib
```
