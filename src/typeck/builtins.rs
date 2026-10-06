//! Declarative registry of builtin methods on primitive and collection
//! receivers (array/map/set/bytes/string/int/float/bool).
//!
//! Before this registry existed, knowledge about these methods was spread
//! across four hand-maintained parallel tables keyed by method-name strings:
//! the typing arms in `infer.rs`, the `is_mutating_builtin` classification in
//! `check.rs`, the lowering dispatch in `codegen/lower/mod.rs`, and the extern
//! declarations in `codegen/runtime.rs`. Adding one method meant editing all
//! four with nothing catching a missed one — e.g. a mutating method left out
//! of `is_mutating_builtin` would silently bypass the strict-mutability rule
//! (#464). Now each method is ONE entry here:
//!
//! - `infer.rs` resolves existence, arity, parameter types and return type
//!   from the entry (including element-type substitution via [`SymTy`]);
//! - `is_mutating_builtin` is a lookup of the `mutating` flag;
//! - the lowering dispatch emits the common call-a-runtime-function shape
//!   from [`Lowering::Runtime`], keeping explicit match arms only for the
//!   few [`Lowering::Inline`] entries with genuinely custom codegen;
//! - `runtime.rs` derives the extern declarations for `Runtime` entries.
//!
//! Deliberately NOT in the registry (they have per-method resolution kinds,
//! error-inference hooks, or value-copy semantics that don't fit the shape):
//! Task methods (`get`/`detach`/`cancel`), channel methods on
//! Sender/Receiver (`send`/`recv`/...), and the `expect()` assertion
//! intrinsics. Analysis passes with their own semantic method lists
//! (`facts.rs` len-bounds, `idempotency.rs`, `dominance.rs`) also stay
//! separate: they classify by *analysis meaning*, not signature.

use super::types::PlutoType;

/// The receiver shapes builtin methods dispatch on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Receiver {
    Array,
    Map,
    Set,
    Bytes,
    Str,
    Int,
    Float,
    Bool,
}

impl Receiver {
    /// The registry receiver kind for a concrete type, if it has one.
    pub(crate) fn of(ty: &PlutoType) -> Option<Receiver> {
        match ty {
            PlutoType::Array(_) => Some(Receiver::Array),
            PlutoType::Map(_, _) => Some(Receiver::Map),
            PlutoType::Set(_) => Some(Receiver::Set),
            PlutoType::Bytes => Some(Receiver::Bytes),
            PlutoType::String => Some(Receiver::Str),
            PlutoType::Int => Some(Receiver::Int),
            PlutoType::Float => Some(Receiver::Float),
            PlutoType::Bool => Some(Receiver::Bool),
            _ => None,
        }
    }

    /// How the receiver is named in "X has no method 'm'" diagnostics.
    pub(crate) fn noun(self) -> &'static str {
        match self {
            Receiver::Array => "array",
            Receiver::Map => "Map",
            Receiver::Set => "Set",
            Receiver::Bytes => "bytes",
            Receiver::Str => "string",
            Receiver::Int => "int",
            Receiver::Float => "float",
            Receiver::Bool => "bool",
        }
    }
}

/// Symbolic type in a builtin signature, resolved against the receiver type
/// (`Elem`/`Key`/`Value` substitute the receiver's generic components).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SymTy {
    Int,
    Float,
    Bool,
    Byte,
    Bytes,
    Str,
    Void,
    /// Array/Set element type.
    Elem,
    /// Map key type.
    Key,
    /// Map value type.
    Value,
    /// The receiver type itself (e.g. `array.slice` returns the same array type).
    SelfTy,
    ArrayOfElem,
    ArrayOfKey,
    ArrayOfValue,
    ArrayOfString,
    NullableInt,
    NullableFloat,
}

impl SymTy {
    /// Resolve against the receiver's concrete type.
    pub(crate) fn resolve(self, recv: &PlutoType) -> PlutoType {
        match self {
            SymTy::Int => PlutoType::Int,
            SymTy::Float => PlutoType::Float,
            SymTy::Bool => PlutoType::Bool,
            SymTy::Byte => PlutoType::Byte,
            SymTy::Bytes => PlutoType::Bytes,
            SymTy::Str => PlutoType::String,
            SymTy::Void => PlutoType::Void,
            SymTy::Elem => match recv {
                PlutoType::Array(e) | PlutoType::Set(e) => (**e).clone(),
                _ => panic!("ICE: SymTy::Elem resolved against non-array/set receiver {recv}"),
            },
            SymTy::Key => match recv {
                PlutoType::Map(k, _) => (**k).clone(),
                _ => panic!("ICE: SymTy::Key resolved against non-map receiver {recv}"),
            },
            SymTy::Value => match recv {
                PlutoType::Map(_, v) => (**v).clone(),
                _ => panic!("ICE: SymTy::Value resolved against non-map receiver {recv}"),
            },
            SymTy::SelfTy => recv.clone(),
            SymTy::ArrayOfElem => PlutoType::Array(Box::new(SymTy::Elem.resolve(recv))),
            SymTy::ArrayOfKey => PlutoType::Array(Box::new(SymTy::Key.resolve(recv))),
            SymTy::ArrayOfValue => PlutoType::Array(Box::new(SymTy::Value.resolve(recv))),
            SymTy::ArrayOfString => PlutoType::Array(Box::new(PlutoType::String)),
            SymTy::NullableInt => PlutoType::Nullable(Box::new(PlutoType::Int)),
            SymTy::NullableFloat => PlutoType::Nullable(Box::new(PlutoType::Float)),
        }
    }
}

/// One parameter of a builtin method.
#[derive(Debug)]
pub(crate) struct Param {
    pub ty: SymTy,
    /// Overrides how the expected type is displayed in a mismatch error,
    /// e.g. `remove_at(): expected int index, found string`.
    pub expected_label: Option<&'static str>,
    /// Names the parameter slot in a mismatch error, e.g. map
    /// `insert() key: expected int, found string`.
    pub slot_label: Option<&'static str>,
    /// Whether codegen materializes string slices to owned strings before
    /// storing (`emit_string_escape`). Only meaningful for `Elem`/`Key`/`Value`
    /// params that are stored into the receiver.
    pub escape: bool,
}

/// Shorthand for a plain parameter.
const fn p(ty: SymTy) -> Param {
    Param { ty, expected_label: None, slot_label: None, escape: false }
}

/// Parameter stored into the receiver (string-escaped before storing).
const fn p_store(ty: SymTy) -> Param {
    Param { ty, expected_label: None, slot_label: None, escape: true }
}

/// Parameter whose mismatch error shows a custom expected label.
const fn p_lbl(ty: SymTy, expected_label: &'static str) -> Param {
    Param { ty, expected_label: Some(expected_label), slot_label: None, escape: false }
}

/// Stored parameter with a named slot in mismatch errors (map insert key/value).
const fn p_slot(ty: SymTy, slot_label: &'static str) -> Param {
    Param { ty, expected_label: None, slot_label: Some(slot_label), escape: true }
}

/// How a wrong-argument-count error is phrased (styles differ historically
/// per receiver family and are preserved exactly).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArityStyle {
    /// `"push() expects 1 argument, got 2"`
    WithGot,
    /// `"insert() expects 2 arguments"`
    Bare,
    /// `"copy_from() expects 4 arguments (src, src_off, dst_off, n)"`
    Note(&'static str),
}

/// Where the runtime hash-table type tag is inserted in the C call.
/// The tag value is `key_type_tag` of the map key (Map) or element
/// type (Array/Set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tag {
    None,
    /// Immediately after the receiver pointer (map/set calls).
    AfterRecv,
    /// After the regular arguments (array contains/index_of).
    AfterArgs,
}

/// How the method lowers to code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lowering {
    /// Call a C runtime function: receiver first, then (per [`Tag`]) a type
    /// tag and the arguments. Argument/return marshaling is derived from the
    /// signature (`Elem`/`Key`/`Value` go through array slots, `Byte` widens
    /// to i64, `Bool` returns reduce from i64, `Void` returns a dummy 0).
    Runtime { symbol: &'static str, tag: Tag },
    /// Custom codegen: an explicit match arm in `lower_method_call`
    /// (e.g. `int.to_float` is a bare fcvt instruction, `array.is_empty`
    /// composes len + icmp). Existence/typing still come from this registry.
    Inline,
}

/// One builtin method: the single source of truth for its existence, typing,
/// mutability classification, and lowering.
#[derive(Debug)]
pub(crate) struct BuiltinMethod {
    pub receiver: Receiver,
    pub name: &'static str,
    pub params: &'static [Param],
    pub ret: SymTy,
    /// Mutates the receiver in place: calling it requires a mutable binding
    /// (same rule as index assignment and `mut self` methods, #395/#464).
    pub mutating: bool,
    pub arity: ArityStyle,
    pub lowering: Lowering,
}

const fn rt(symbol: &'static str) -> Lowering {
    Lowering::Runtime { symbol, tag: Tag::None }
}

const fn rt_tag(symbol: &'static str, tag: Tag) -> Lowering {
    Lowering::Runtime { symbol, tag }
}

macro_rules! entry {
    ($recv:ident, $name:literal, [$($param:expr),*], $ret:ident, mutating: $mut:literal, $arity:expr, $lowering:expr) => {
        BuiltinMethod {
            receiver: Receiver::$recv,
            name: $name,
            params: &[$($param),*],
            ret: SymTy::$ret,
            mutating: $mut,
            arity: $arity,
            lowering: $lowering,
        }
    };
}

use ArityStyle::{Bare, Note, WithGot};

/// Every builtin method on a primitive/collection receiver.
pub(crate) static BUILTIN_METHODS: &[BuiltinMethod] = &[
    // ---- array ----
    entry!(Array, "len", [], Int, mutating: false, WithGot, rt("__pluto_array_len")),
    entry!(Array, "push", [p_store(SymTy::Elem)], Void, mutating: true, WithGot, rt("__pluto_array_push")),
    entry!(Array, "pop", [], Elem, mutating: true, WithGot, rt("__pluto_array_pop")),
    entry!(Array, "last", [], Elem, mutating: false, WithGot, rt("__pluto_array_last")),
    entry!(Array, "first", [], Elem, mutating: false, WithGot, rt("__pluto_array_first")),
    entry!(Array, "is_empty", [], Bool, mutating: false, WithGot, Lowering::Inline), // len() == 0
    entry!(Array, "clear", [], Void, mutating: true, WithGot, rt("__pluto_array_clear")),
    entry!(Array, "reverse", [], Void, mutating: true, WithGot, rt("__pluto_array_reverse")),
    entry!(Array, "remove_at", [p_lbl(SymTy::Int, "int index")], Elem, mutating: true, WithGot, rt("__pluto_array_remove_at")),
    entry!(Array, "insert_at", [p_lbl(SymTy::Int, "int index"), p(SymTy::Elem)], Void, mutating: true, WithGot, rt("__pluto_array_insert_at")),
    entry!(Array, "slice", [p_lbl(SymTy::Int, "int start"), p_lbl(SymTy::Int, "int end")], SelfTy, mutating: false, WithGot, rt("__pluto_array_slice")),
    entry!(Array, "contains", [p(SymTy::Elem)], Bool, mutating: false, WithGot, rt_tag("__pluto_array_contains", Tag::AfterArgs)),
    entry!(Array, "index_of", [p(SymTy::Elem)], Int, mutating: false, WithGot, rt_tag("__pluto_array_index_of", Tag::AfterArgs)),
    // ---- map ----
    entry!(Map, "len", [], Int, mutating: false, Bare, rt("__pluto_map_len")),
    entry!(Map, "contains", [p(SymTy::Key)], Bool, mutating: false, Bare, rt_tag("__pluto_map_contains", Tag::AfterRecv)),
    entry!(Map, "insert", [p_slot(SymTy::Key, "key"), p_slot(SymTy::Value, "value")], Void, mutating: true, Bare, rt_tag("__pluto_map_insert", Tag::AfterRecv)),
    entry!(Map, "remove", [p(SymTy::Key)], Void, mutating: true, Bare, rt_tag("__pluto_map_remove", Tag::AfterRecv)),
    entry!(Map, "keys", [], ArrayOfKey, mutating: false, Bare, rt("__pluto_map_keys")),
    entry!(Map, "values", [], ArrayOfValue, mutating: false, Bare, rt("__pluto_map_values")),
    // ---- set ----
    entry!(Set, "len", [], Int, mutating: false, Bare, rt("__pluto_set_len")),
    entry!(Set, "contains", [p(SymTy::Elem)], Bool, mutating: false, Bare, rt_tag("__pluto_set_contains", Tag::AfterRecv)),
    entry!(Set, "insert", [p_store(SymTy::Elem)], Void, mutating: true, Bare, rt_tag("__pluto_set_insert", Tag::AfterRecv)),
    entry!(Set, "remove", [p(SymTy::Elem)], Void, mutating: true, Bare, rt_tag("__pluto_set_remove", Tag::AfterRecv)),
    entry!(Set, "to_array", [], ArrayOfElem, mutating: false, Bare, rt("__pluto_set_to_array")),
    // ---- bytes ----
    entry!(Bytes, "len", [], Int, mutating: false, Bare, rt("__pluto_bytes_len")),
    entry!(Bytes, "push", [p(SymTy::Byte)], Void, mutating: true, Bare, rt("__pluto_bytes_push")),
    entry!(Bytes, "to_string", [], Str, mutating: false, Bare, rt("__pluto_bytes_to_string")),
    entry!(Bytes, "slice", [p(SymTy::Int), p(SymTy::Int)], Bytes, mutating: false, Note("start, end"), rt("__pluto_bytes_slice")),
    entry!(Bytes, "extend", [p(SymTy::Bytes)], Void, mutating: true, Bare, rt("__pluto_bytes_extend")),
    entry!(Bytes, "fill", [p(SymTy::Byte)], Void, mutating: true, Bare, rt("__pluto_bytes_fill")),
    entry!(Bytes, "copy_from", [p(SymTy::Bytes), p(SymTy::Int), p(SymTy::Int), p(SymTy::Int)], Void, mutating: true, Note("src, src_off, dst_off, n"), rt("__pluto_bytes_copy_from")),
    entry!(Bytes, "find", [p(SymTy::Byte), p(SymTy::Int)], Int, mutating: false, Note("needle, from"), rt("__pluto_bytes_find")),
    entry!(Bytes, "compare", [p(SymTy::Bytes)], Int, mutating: false, Bare, rt("__pluto_bytes_compare")),
    entry!(Bytes, "read_u8", [p(SymTy::Int)], Int, mutating: false, Note("offset"), rt("__pluto_bytes_read_u8")),
    entry!(Bytes, "read_u16_le", [p(SymTy::Int)], Int, mutating: false, Note("offset"), rt("__pluto_bytes_read_u16_le")),
    entry!(Bytes, "read_u16_be", [p(SymTy::Int)], Int, mutating: false, Note("offset"), rt("__pluto_bytes_read_u16_be")),
    entry!(Bytes, "read_u32_le", [p(SymTy::Int)], Int, mutating: false, Note("offset"), rt("__pluto_bytes_read_u32_le")),
    entry!(Bytes, "read_u32_be", [p(SymTy::Int)], Int, mutating: false, Note("offset"), rt("__pluto_bytes_read_u32_be")),
    entry!(Bytes, "read_i64_le", [p(SymTy::Int)], Int, mutating: false, Note("offset"), rt("__pluto_bytes_read_i64_le")),
    entry!(Bytes, "read_i64_be", [p(SymTy::Int)], Int, mutating: false, Note("offset"), rt("__pluto_bytes_read_i64_be")),
    entry!(Bytes, "write_u8", [p(SymTy::Int), p(SymTy::Int)], Void, mutating: true, Note("offset, value"), rt("__pluto_bytes_write_u8")),
    entry!(Bytes, "write_u16_le", [p(SymTy::Int), p(SymTy::Int)], Void, mutating: true, Note("offset, value"), rt("__pluto_bytes_write_u16_le")),
    entry!(Bytes, "write_u16_be", [p(SymTy::Int), p(SymTy::Int)], Void, mutating: true, Note("offset, value"), rt("__pluto_bytes_write_u16_be")),
    entry!(Bytes, "write_u32_le", [p(SymTy::Int), p(SymTy::Int)], Void, mutating: true, Note("offset, value"), rt("__pluto_bytes_write_u32_le")),
    entry!(Bytes, "write_u32_be", [p(SymTy::Int), p(SymTy::Int)], Void, mutating: true, Note("offset, value"), rt("__pluto_bytes_write_u32_be")),
    entry!(Bytes, "write_i64_le", [p(SymTy::Int), p(SymTy::Int)], Void, mutating: true, Note("offset, value"), rt("__pluto_bytes_write_i64_le")),
    entry!(Bytes, "write_i64_be", [p(SymTy::Int), p(SymTy::Int)], Void, mutating: true, Note("offset, value"), rt("__pluto_bytes_write_i64_be")),
    // ---- int ----
    entry!(Int, "to_string", [], Str, mutating: false, WithGot, rt("__pluto_int_to_string")),
    entry!(Int, "to_float", [], Float, mutating: false, WithGot, Lowering::Inline), // bare fcvt_from_sint
    entry!(Int, "abs", [], Int, mutating: false, WithGot, rt("__pluto_abs_int")),
    // ---- float ----
    entry!(Float, "to_string", [], Str, mutating: false, WithGot, rt("__pluto_float_to_string")),
    entry!(Float, "to_int", [], Int, mutating: false, WithGot, Lowering::Inline), // bare fcvt_to_sint_sat
    entry!(Float, "abs", [], Float, mutating: false, WithGot, rt("__pluto_abs_float")),
    entry!(Float, "sqrt", [], Float, mutating: false, WithGot, rt("__pluto_sqrt")),
    entry!(Float, "floor", [], Float, mutating: false, WithGot, rt("__pluto_floor")),
    entry!(Float, "ceil", [], Float, mutating: false, WithGot, rt("__pluto_ceil")),
    entry!(Float, "round", [], Float, mutating: false, WithGot, rt("__pluto_round")),
    // ---- bool ----
    entry!(Bool, "to_string", [], Str, mutating: false, WithGot, Lowering::Inline), // receiver widens I8 -> I32 for the C ABI
    // ---- string ----
    entry!(Str, "len", [], Int, mutating: false, Bare, rt("__pluto_string_len")),
    entry!(Str, "trim", [], Str, mutating: false, Bare, rt("__pluto_string_trim")),
    entry!(Str, "to_upper", [], Str, mutating: false, Bare, rt("__pluto_string_to_upper")),
    entry!(Str, "to_lower", [], Str, mutating: false, Bare, rt("__pluto_string_to_lower")),
    entry!(Str, "contains", [p(SymTy::Str)], Bool, mutating: false, Bare, rt("__pluto_string_contains")),
    entry!(Str, "starts_with", [p(SymTy::Str)], Bool, mutating: false, Bare, rt("__pluto_string_starts_with")),
    entry!(Str, "ends_with", [p(SymTy::Str)], Bool, mutating: false, Bare, rt("__pluto_string_ends_with")),
    entry!(Str, "index_of", [p(SymTy::Str)], Int, mutating: false, Bare, rt("__pluto_string_index_of")),
    entry!(Str, "char_at", [p(SymTy::Int)], Str, mutating: false, Bare, rt("__pluto_string_char_at")),
    entry!(Str, "byte_at", [p(SymTy::Int)], Int, mutating: false, Bare, rt("__pluto_string_byte_at")),
    entry!(Str, "substring", [p(SymTy::Int), p(SymTy::Int)], Str, mutating: false, Bare, rt("__pluto_string_substring")),
    entry!(Str, "replace", [p(SymTy::Str), p(SymTy::Str)], Str, mutating: false, Bare, rt("__pluto_string_replace")),
    entry!(Str, "split", [p(SymTy::Str)], ArrayOfString, mutating: false, Bare, rt("__pluto_string_split")),
    entry!(Str, "to_int", [], NullableInt, mutating: false, Bare, rt("__pluto_string_to_int")),
    entry!(Str, "to_float", [], NullableFloat, mutating: false, Bare, rt("__pluto_string_to_float")),
    entry!(Str, "to_bytes", [], Bytes, mutating: false, Bare, rt("__pluto_string_to_bytes")),
    entry!(Str, "trim_start", [], Str, mutating: false, Bare, rt("__pluto_string_trim_start")),
    entry!(Str, "trim_end", [], Str, mutating: false, Bare, rt("__pluto_string_trim_end")),
    entry!(Str, "repeat", [p(SymTy::Int)], Str, mutating: false, Bare, rt("__pluto_string_repeat")),
    entry!(Str, "last_index_of", [p(SymTy::Str)], Int, mutating: false, Bare, rt("__pluto_string_last_index_of")),
    entry!(Str, "count", [p(SymTy::Str)], Int, mutating: false, Bare, rt("__pluto_string_count")),
    entry!(Str, "is_empty", [], Bool, mutating: false, Bare, rt("__pluto_string_is_empty")),
    entry!(Str, "is_whitespace", [], Bool, mutating: false, Bare, rt("__pluto_string_is_whitespace")),
];

/// Look up a builtin method by receiver kind and name.
pub(crate) fn lookup(receiver: Receiver, name: &str) -> Option<&'static BuiltinMethod> {
    BUILTIN_METHODS
        .iter()
        .find(|m| m.receiver == receiver && m.name == name)
}

/// Look up a builtin method for a concrete receiver type.
pub(crate) fn lookup_for_type(ty: &PlutoType, name: &str) -> Option<&'static BuiltinMethod> {
    Receiver::of(ty).and_then(|r| lookup(r, name))
}

/// The type whose `key_type_tag` is passed for [`Tag::AfterRecv`]/
/// [`Tag::AfterArgs`] calls: the key type for maps, the element type for
/// arrays and sets.
pub(crate) fn tag_source_type(recv: &PlutoType) -> PlutoType {
    match recv {
        PlutoType::Map(k, _) => (**k).clone(),
        PlutoType::Array(e) | PlutoType::Set(e) => (**e).clone(),
        _ => panic!("ICE: tag_source_type on non-collection receiver {recv}"),
    }
}

/// Wrong-argument-count message, preserving each receiver family's
/// historical phrasing.
pub(crate) fn arity_error_msg(method: &str, style: ArityStyle, expected: usize, got: usize) -> String {
    let noun = if expected == 1 { "argument" } else { "arguments" };
    match style {
        ArityStyle::WithGot => format!("{method}() expects {expected} {noun}, got {got}"),
        ArityStyle::Bare => format!("{method}() expects {expected} {noun}"),
        ArityStyle::Note(note) => format!("{method}() expects {expected} {noun} ({note})"),
    }
}

/// Argument type-mismatch message for one parameter.
pub(crate) fn param_mismatch_msg(
    method: &str,
    param: &Param,
    expected: &PlutoType,
    found: &PlutoType,
) -> String {
    let shown: String = match param.expected_label {
        Some(label) => label.to_string(),
        None => expected.to_string(),
    };
    match param.slot_label {
        Some(slot) => format!("{method}() {slot}: expected {shown}, found {found}"),
        None => format!("{method}(): expected {shown}, found {found}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each (receiver, name) pair resolves to exactly one entry — the
    /// registry is a function, not a multimap.
    #[test]
    fn no_duplicate_entries() {
        let mut seen = std::collections::HashSet::new();
        for m in BUILTIN_METHODS {
            assert!(
                seen.insert((m.receiver, m.name)),
                "duplicate builtin method entry: {:?}.{}",
                m.receiver,
                m.name
            );
        }
    }

    /// Mutating methods only make sense on mutable container receivers:
    /// int/float/bool/string are immutable values, so a mutating entry on
    /// one of them would be a registry bug (and would trip the
    /// strict-mutability rule on types it can never apply to).
    #[test]
    fn mutating_implies_mutable_container_receiver() {
        for m in BUILTIN_METHODS {
            if m.mutating {
                assert!(
                    matches!(
                        m.receiver,
                        Receiver::Array | Receiver::Map | Receiver::Set | Receiver::Bytes
                    ),
                    "mutating builtin {:?}.{} has a non-container receiver",
                    m.receiver,
                    m.name
                );
            }
        }
    }

    /// Runtime-lowered entries name a pluto runtime symbol; collection
    /// tag-passing only occurs where the runtime hash tables need it.
    #[test]
    fn runtime_symbols_well_formed() {
        for m in BUILTIN_METHODS {
            if let Lowering::Runtime { symbol, tag } = m.lowering {
                assert!(
                    symbol.starts_with("__pluto_"),
                    "{:?}.{} lowers to non-runtime symbol {symbol}",
                    m.receiver,
                    m.name
                );
                if tag != Tag::None {
                    assert!(
                        matches!(m.receiver, Receiver::Array | Receiver::Map | Receiver::Set),
                        "{:?}.{} passes a key-type tag but is not a tagged collection call",
                        m.receiver,
                        m.name
                    );
                }
            }
        }
    }

    /// Mutating methods return void or the removed element — never a fresh
    /// view of the receiver (those are the read-only methods).
    #[test]
    fn mutating_methods_return_void_or_element() {
        for m in BUILTIN_METHODS {
            if m.mutating {
                assert!(
                    matches!(m.ret, SymTy::Void | SymTy::Elem),
                    "mutating builtin {:?}.{} returns {:?}",
                    m.receiver,
                    m.name,
                    m.ret
                );
            }
        }
    }

    /// The historical `is_mutating_builtin` classification (check.rs, #464):
    /// pin the exact mutating set so a mis-flagged entry is caught here
    /// rather than silently relaxing or tightening the mutability rule.
    #[test]
    fn mutating_set_matches_original_table() {
        let mutating: Vec<(Receiver, &str)> = BUILTIN_METHODS
            .iter()
            .filter(|m| m.mutating)
            .map(|m| (m.receiver, m.name))
            .collect();
        let expected: Vec<(Receiver, &str)> = vec![
            (Receiver::Array, "push"),
            (Receiver::Array, "pop"),
            (Receiver::Array, "clear"),
            (Receiver::Array, "reverse"),
            (Receiver::Array, "remove_at"),
            (Receiver::Array, "insert_at"),
            (Receiver::Map, "insert"),
            (Receiver::Map, "remove"),
            (Receiver::Set, "insert"),
            (Receiver::Set, "remove"),
            (Receiver::Bytes, "push"),
            (Receiver::Bytes, "extend"),
            (Receiver::Bytes, "fill"),
            (Receiver::Bytes, "copy_from"),
            (Receiver::Bytes, "write_u8"),
            (Receiver::Bytes, "write_u16_le"),
            (Receiver::Bytes, "write_u16_be"),
            (Receiver::Bytes, "write_u32_le"),
            (Receiver::Bytes, "write_u32_be"),
            (Receiver::Bytes, "write_i64_le"),
            (Receiver::Bytes, "write_i64_be"),
        ];
        assert_eq!(mutating, expected);
    }
}
