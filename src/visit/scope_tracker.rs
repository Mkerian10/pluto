/// Scope tracking utility for managing nested scopes during AST traversal.
///
/// Many compiler passes need to track scope-local state (variables, types, bindings)
/// with proper nesting semantics. This utility provides a clean abstraction over the
/// common pattern of maintaining a stack of scopes.
///
/// # Examples
///
/// ## Basic usage
///
/// ```rust
/// use pluto::visit::scope_tracker::ScopeTracker;
/// use pluto::typeck::types::PlutoType;
///
/// let mut tracker = ScopeTracker::<PlutoType>::new();
///
/// // Enter function scope
/// tracker.push_scope();
/// tracker.insert("x".to_string(), PlutoType::Int);
///
/// // Enter nested block
/// tracker.push_scope();
/// tracker.insert("y".to_string(), PlutoType::Float);
///
/// // Lookup searches from innermost to outermost
/// assert_eq!(tracker.lookup("y"), Some(&PlutoType::Float));
/// assert_eq!(tracker.lookup("x"), Some(&PlutoType::Int));
///
/// // Exit block
/// tracker.pop_scope();
/// assert_eq!(tracker.lookup("y"), None);
/// assert_eq!(tracker.lookup("x"), Some(&PlutoType::Int));
/// ```
///
/// ## With depth tracking
///
/// ```rust
/// use pluto::visit::scope_tracker::ScopeTracker;
///
/// let mut tracker = ScopeTracker::<i32>::new();
/// tracker.push_scope();
/// tracker.insert("x".to_string(), 1);
/// tracker.push_scope();
/// tracker.insert("y".to_string(), 2);
///
/// // Depth 0 = outermost, 1 = inner
/// assert_eq!(tracker.lookup_with_depth("x"), Some((&1, 0)));
/// assert_eq!(tracker.lookup_with_depth("y"), Some((&2, 1)));
/// ```

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ScopeTracker<T> {
    scopes: Vec<HashMap<String, T>>,
}

impl<T> ScopeTracker<T> {
    /// Create a new scope tracker with no scopes.
    ///
    /// Call `push_scope()` to create the first scope before inserting values.
    pub fn new() -> Self {
        Self { scopes: Vec::new() }
    }

    /// Create a new scope tracker with an initial scope.
    pub fn with_initial_scope() -> Self {
        let mut tracker = Self::new();
        tracker.push_scope();
        tracker
    }

    /// Push a new scope onto the stack.
    ///
    /// All subsequent `insert()` calls will add to this new scope until `pop_scope()`.
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pop the innermost scope from the stack, returning its contents.
    ///
    /// Returns `None` if there are no scopes to pop.
    pub fn pop_scope(&mut self) -> Option<HashMap<String, T>> {
        self.scopes.pop()
    }

    /// Insert a binding into the current (innermost) scope.
    ///
    /// # Panics
    ///
    /// Panics if there are no scopes (call `push_scope()` first).
    pub fn insert(&mut self, name: String, value: T) {
        self.scopes
            .last_mut()
            .expect("ScopeTracker::insert called with no active scope")
            .insert(name, value);
    }

    /// Insert a binding into the current scope, returning the previous value if it existed.
    ///
    /// This is useful for detecting shadowing in the same scope.
    ///
    /// # Panics
    ///
    /// Panics if there are no scopes.
    pub fn insert_shadowing(&mut self, name: String, value: T) -> Option<T> {
        self.scopes
            .last_mut()
            .expect("ScopeTracker::insert_shadowing called with no active scope")
            .insert(name, value)
    }

    /// Look up a binding, searching from innermost to outermost scope.
    ///
    /// Returns `None` if the binding is not found in any scope.
    pub fn lookup(&self, name: &str) -> Option<&T> {
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.get(name) {
                return Some(value);
            }
        }
        None
    }

    /// Look up a binding mutably, searching from innermost to outermost scope.
    pub fn lookup_mut(&mut self, name: &str) -> Option<&mut T> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(value) = scope.get_mut(name) {
                return Some(value);
            }
        }
        None
    }

    /// Look up a binding with depth information.
    ///
    /// Returns `Some((value, depth))` where depth is the scope index (0 = outermost).
    pub fn lookup_with_depth(&self, name: &str) -> Option<(&T, usize)> {
        for (i, scope) in self.scopes.iter().enumerate().rev() {
            if let Some(value) = scope.get(name) {
                return Some((value, i));
            }
        }
        None
    }

    /// Check if a binding exists in any scope.
    pub fn contains(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    /// Check if a binding exists in the current (innermost) scope only.
    pub fn contains_in_current(&self, name: &str) -> bool {
        self.scopes
            .last()
            .map(|scope| scope.contains_key(name))
            .unwrap_or(false)
    }

    /// Get a reference to the current (innermost) scope.
    pub fn current_scope(&self) -> Option<&HashMap<String, T>> {
        self.scopes.last()
    }

    /// Get a mutable reference to the current (innermost) scope.
    pub fn current_scope_mut(&mut self) -> Option<&mut HashMap<String, T>> {
        self.scopes.last_mut()
    }

    /// Get the current scope depth (number of active scopes).
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// Check if the tracker has no scopes.
    pub fn is_empty(&self) -> bool {
        self.scopes.is_empty()
    }

    /// Remove all scopes, resetting to empty state.
    pub fn clear(&mut self) {
        self.scopes.clear();
    }
}

impl<T> Default for ScopeTracker<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ==============================================================================
// Tests
// ==============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_insert_lookup() {
        let mut tracker = ScopeTracker::new();
        tracker.push_scope();
        tracker.insert("x".to_string(), 42);
        assert_eq!(tracker.lookup("x"), Some(&42));
        assert_eq!(tracker.lookup("y"), None);
    }

    #[test]
    fn test_nested_scopes() {
        let mut tracker = ScopeTracker::new();
        tracker.push_scope();
        tracker.insert("x".to_string(), 1);

        tracker.push_scope();
        tracker.insert("y".to_string(), 2);

        assert_eq!(tracker.lookup("x"), Some(&1));
        assert_eq!(tracker.lookup("y"), Some(&2));

        tracker.pop_scope();
        assert_eq!(tracker.lookup("x"), Some(&1));
        assert_eq!(tracker.lookup("y"), None);
    }

    #[test]
    fn test_shadowing() {
        let mut tracker = ScopeTracker::new();
        tracker.push_scope();
        tracker.insert("x".to_string(), 1);

        tracker.push_scope();
        tracker.insert("x".to_string(), 2);

        assert_eq!(tracker.lookup("x"), Some(&2)); // Inner shadows outer

        tracker.pop_scope();
        assert_eq!(tracker.lookup("x"), Some(&1)); // Outer visible again
    }

    #[test]
    fn test_lookup_with_depth() {
        let mut tracker = ScopeTracker::new();
        tracker.push_scope();
        tracker.insert("x".to_string(), 1);

        tracker.push_scope();
        tracker.insert("y".to_string(), 2);

        assert_eq!(tracker.lookup_with_depth("x"), Some((&1, 0)));
        assert_eq!(tracker.lookup_with_depth("y"), Some((&2, 1)));
        assert_eq!(tracker.lookup_with_depth("z"), None);
    }

    #[test]
    fn test_contains() {
        let mut tracker = ScopeTracker::new();
        tracker.push_scope();
        tracker.insert("x".to_string(), 1);

        assert!(tracker.contains("x"));
        assert!(!tracker.contains("y"));
    }

    #[test]
    fn test_contains_in_current() {
        let mut tracker = ScopeTracker::new();
        tracker.push_scope();
        tracker.insert("x".to_string(), 1);

        tracker.push_scope();
        tracker.insert("y".to_string(), 2);

        assert!(tracker.contains_in_current("y"));
        assert!(!tracker.contains_in_current("x")); // x is in outer scope
    }

    #[test]
    fn test_current_scope() {
        let mut tracker = ScopeTracker::new();
        assert!(tracker.current_scope().is_none());

        tracker.push_scope();
        tracker.insert("x".to_string(), 1);

        let scope = tracker.current_scope().unwrap();
        assert_eq!(scope.get("x"), Some(&1));
    }

    #[test]
    fn test_depth() {
        let mut tracker = ScopeTracker::<i32>::new();
        assert_eq!(tracker.depth(), 0);

        tracker.push_scope();
        assert_eq!(tracker.depth(), 1);

        tracker.push_scope();
        assert_eq!(tracker.depth(), 2);

        tracker.pop_scope();
        assert_eq!(tracker.depth(), 1);
    }

    #[test]
    fn test_is_empty() {
        let mut tracker = ScopeTracker::<i32>::new();
        assert!(tracker.is_empty());

        tracker.push_scope();
        assert!(!tracker.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut tracker = ScopeTracker::new();
        tracker.push_scope();
        tracker.insert("x".to_string(), 1);
        tracker.push_scope();
        tracker.insert("y".to_string(), 2);

        tracker.clear();
        assert!(tracker.is_empty());
        assert_eq!(tracker.depth(), 0);
    }

    #[test]
    fn test_pop_scope_returns_contents() {
        let mut tracker = ScopeTracker::new();
        tracker.push_scope();
        tracker.insert("x".to_string(), 1);
        tracker.insert("y".to_string(), 2);

        let scope = tracker.pop_scope().unwrap();
        assert_eq!(scope.get("x"), Some(&1));
        assert_eq!(scope.get("y"), Some(&2));
    }

    #[test]
    fn test_insert_shadowing() {
        let mut tracker = ScopeTracker::new();
        tracker.push_scope();

        let prev = tracker.insert_shadowing("x".to_string(), 1);
        assert_eq!(prev, None);

        let prev = tracker.insert_shadowing("x".to_string(), 2);
        assert_eq!(prev, Some(1));

        assert_eq!(tracker.lookup("x"), Some(&2));
    }

    #[test]
    fn test_lookup_mut() {
        let mut tracker = ScopeTracker::new();
        tracker.push_scope();
        tracker.insert("x".to_string(), 1);

        if let Some(value) = tracker.lookup_mut("x") {
            *value = 42;
        }

        assert_eq!(tracker.lookup("x"), Some(&42));
    }

    #[test]
    fn test_with_initial_scope() {
        let mut tracker = ScopeTracker::<i32>::with_initial_scope();
        assert_eq!(tracker.depth(), 1);
        assert!(!tracker.is_empty());

        tracker.insert("x".to_string(), 42);
        assert_eq!(tracker.lookup("x"), Some(&42));
    }

    #[test]
    #[should_panic(expected = "ScopeTracker::insert called with no active scope")]
    fn test_insert_without_scope_panics() {
        let mut tracker = ScopeTracker::<i32>::new();
        tracker.insert("x".to_string(), 42);
    }

    #[test]
    fn test_multiple_scope_levels() {
        let mut tracker = ScopeTracker::new();

        // Level 0
        tracker.push_scope();
        tracker.insert("a".to_string(), 1);

        // Level 1
        tracker.push_scope();
        tracker.insert("b".to_string(), 2);

        // Level 2
        tracker.push_scope();
        tracker.insert("c".to_string(), 3);

        // All visible
        assert_eq!(tracker.lookup("a"), Some(&1));
        assert_eq!(tracker.lookup("b"), Some(&2));
        assert_eq!(tracker.lookup("c"), Some(&3));

        // Pop level 2
        tracker.pop_scope();
        assert_eq!(tracker.lookup("c"), None);
        assert_eq!(tracker.lookup("b"), Some(&2));

        // Pop level 1
        tracker.pop_scope();
        assert_eq!(tracker.lookup("b"), None);
        assert_eq!(tracker.lookup("a"), Some(&1));
    }
}
