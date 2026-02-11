# Phase 2: Local Service Model Implementation

## Status: IN PROGRESS

## Changes Made:

### TypeEnv (src/typeck/env.rs):
- [ ] Add CrossStageCall struct
- [ ] Add current_stage field
- [ ] Add stage_names field
- [ ] Add cross_stage_calls field
- [ ] Add pub_methods field
- [ ] Initialize all fields in TypeEnv::new()

### Stage Registration (src/typeck/register.rs):
- [ ] Track stage names in register_stage_placeholders
- [ ] Track pub methods when registering stage methods
- [ ] Set current_stage context when typechecking stage methods

### Cross-Stage Detection (src/typeck/infer.rs):
- [ ] Detect cross-stage calls in infer_method_call
- [ ] Enforce pub visibility for cross-stage calls

### Tests (tests/integration/stages.rs):
- [ ] Test cross-stage call to pub method succeeds
- [ ] Test cross-stage call to private method fails
- [ ] Test same-stage call to private method succeeds
- [ ] Test multiple cross-stage calls with mixed visibility

## Next Steps:
1. Complete all TypeEnv changes
2. Complete register.rs changes
3. Complete infer.rs changes
4. Add tests
5. Run tests to verify implementation
