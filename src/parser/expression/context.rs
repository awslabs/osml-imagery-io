//! The shared evaluator context: a tree of named nodes.
//!
//! [`EvalContext`] is the read/write-neutral navigation context the
//! [`ExpressionEvaluator`](super::eval::ExpressionEvaluator) walks. It is a tree
//! of named [`Node`]s rather than a flat scalar map, so the postfix `[]`
//! subscript and `.member` access can traverse maps, arrays, and structs.
//!
//! # Roles
//!
//! [`Node`] is a distinct third role, deliberately **not** a unification of the
//! read path's [`Value`](crate::parser::value::Value) (borrowed parse-output) or
//! the write path's [`WriteValue`](crate::parser::writer::WriteValue) (owned
//! serialization currency). It is owned/`'static` so the inherited-scope snapshot
//! machinery (`eval_snapshot`/`set_inherited_context`/`new_with_inherited`) can
//! hand context values *across* accessor/writer lifetimes.
//!
//! # Two-tier value split
//!
//! [`EvalResult`] holds only what can be a valid *final* result of an expression
//! — scalars. Maps, arrays, and structs are **intermediary** navigation nodes
//! that `[]` / `.member` traverse; they are never a final result. Navigation that
//! lands on a `Scalar` yields the value ops and methods consume; a bare
//! intermediary at the size/repeat/condition boundary is a type error.
//!
//! In this phase (the shared-context refactor) only `Scalar` leaves are populated
//! on both paths — the tree mirrors the flat scalar scope it replaces. The `Map`
//! node is seeded by the `consts:` phase, and `Array`/`Struct` nodes by the
//! array-subscript phase.

use std::collections::HashMap;

use super::EvalResult;

/// A node in the evaluation context tree.
///
/// `Scalar` is a leaf (the only thing an expression can evaluate to). `Map`,
/// `Array`, and `Struct` are **intermediary** nodes that navigation operators
/// (`[]`, `.member`) traverse; they never reach an operator or method arm,
/// because navigation resolves to a `Scalar` before a value is consumed.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// Leaf value — reuses the scalar result type.
    Scalar(EvalResult),
    /// A `consts:` lookup table (string-keyed). Seeded by the const-map phase.
    Map(HashMap<String, EvalResult>),
    /// A repeated group. Populated by the array-subscript phase.
    Array(Vec<Node>),
    /// A nested type instance. Populated by the array-subscript phase.
    Struct(HashMap<String, Node>),
}

impl Node {
    /// Borrow the inner scalar if this node is a `Scalar` leaf.
    ///
    /// Returns `None` for intermediary nodes (`Map`/`Array`/`Struct`), which a
    /// caller must navigate through rather than consume directly.
    pub fn as_scalar(&self) -> Option<&EvalResult> {
        match self {
            Node::Scalar(v) => Some(v),
            _ => None,
        }
    }
}

/// Context for expression evaluation: a tree of named nodes plus the innermost
/// repeat index.
///
/// The root maps a field/const name to a [`Node`]. Scalar leaves are the
/// parsed/written field values and scalar consts; intermediary nodes are the
/// const maps, repeated groups, and nested-type instances that navigation
/// traverses.
#[derive(Debug, Clone, Default)]
pub struct EvalContext {
    /// Named nodes at this scope.
    root: HashMap<String, Node>,
    /// Current repetition index (for `_index`).
    pub index: Option<usize>,
}

impl EvalContext {
    /// Create a new empty evaluation context.
    pub fn new() -> Self {
        Self {
            root: HashMap::new(),
            index: None,
        }
    }

    /// Set a scalar field value (builder style).
    pub fn with_field(mut self, path: impl Into<String>, value: EvalResult) -> Self {
        self.root.insert(path.into(), Node::Scalar(value));
        self
    }

    /// Set the current index (builder style).
    pub fn with_index(mut self, index: usize) -> Self {
        self.index = Some(index);
        self
    }

    /// Insert a scalar leaf — the common case for parsed/written field values.
    pub fn insert_scalar(&mut self, name: impl Into<String>, value: EvalResult) {
        self.root.insert(name.into(), Node::Scalar(value));
    }

    /// Insert an arbitrary node (a `Map`/`Array`/`Struct` intermediary).
    pub fn insert_node(&mut self, name: impl Into<String>, node: Node) {
        self.root.insert(name.into(), node);
    }

    /// Look up a node by name.
    pub fn get(&self, name: &str) -> Option<&Node> {
        self.root.get(name)
    }

    /// Look up a scalar leaf by name.
    ///
    /// Returns `None` if the name is absent **or** resolves to an intermediary
    /// node (a bare `Map`/`Array`/`Struct` cannot be a final value).
    pub fn get_scalar(&self, name: &str) -> Option<&EvalResult> {
        self.root.get(name).and_then(Node::as_scalar)
    }

    /// Whether a name is bound at this scope (scalar or intermediary).
    pub fn contains(&self, name: &str) -> bool {
        self.root.contains_key(name)
    }

    /// Snapshot the scope's nodes as a flat map for a nested sub-accessor/
    /// sub-writer to inherit.
    ///
    /// Scalar leaves **and** navigable `Array`/`Struct` intermediaries are
    /// carried across the nesting boundary so a nested expression like
    /// `IMAGE_RECORDS[image_index].NCOLCB` — which indexes an *enclosing*
    /// repeated group from inside a nested type — resolves. `Node` is
    /// owned/`'static` (Decision 2), so these clones cross accessor/writer
    /// lifetimes freely.
    ///
    /// Const `Map` nodes are **not** carried: they are re-seeded into every
    /// scope from `definition.consts` (the loader stamps file-level consts onto
    /// each nested type), so threading them here would be redundant.
    pub fn snapshot(&self) -> HashMap<String, Node> {
        self.root
            .iter()
            .filter(|(_, n)| !matches!(n, Node::Map(_)))
            .map(|(k, n)| (k.clone(), n.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_round_trips_through_node() {
        let mut ctx = EvalContext::new();
        ctx.insert_scalar("N", EvalResult::Integer(7));
        assert_eq!(ctx.get_scalar("N"), Some(&EvalResult::Integer(7)));
        assert!(ctx.contains("N"));
    }

    #[test]
    fn intermediary_node_is_not_a_scalar() {
        // A Map/Array/Struct node exists in the tree but is not a valid final
        // value: get_scalar must return None so the size/repeat/condition
        // boundary treats a bare intermediary as absent-from-scalars.
        let mut ctx = EvalContext::new();
        let mut table = HashMap::new();
        table.insert("06a".to_string(), EvalResult::Integer(11));
        ctx.insert_node("widths", Node::Map(table));
        assert!(ctx.get("widths").is_some());
        assert_eq!(ctx.get_scalar("widths"), None);
    }

    #[test]
    fn snapshot_carries_scalars_and_arrays_but_not_const_maps() {
        // The inherited snapshot threads scalar leaves and navigable
        // Array/Struct intermediaries across a nesting boundary (so an enclosing
        // repeated group is indexable from a nested type), but drops const `Map`
        // nodes, which every scope re-seeds from `definition.consts`.
        let mut ctx = EvalContext::new();
        ctx.insert_scalar("N", EvalResult::Integer(1));
        ctx.insert_node(
            "arr",
            Node::Array(vec![Node::Scalar(EvalResult::Integer(2))]),
        );
        let mut table = HashMap::new();
        table.insert("06a".to_string(), EvalResult::Integer(11));
        ctx.insert_node("widths", Node::Map(table));
        let snap = ctx.snapshot();
        assert_eq!(snap.get("N"), Some(&Node::Scalar(EvalResult::Integer(1))));
        assert!(snap.contains_key("arr"));
        assert!(!snap.contains_key("widths"));
    }

    #[test]
    fn as_scalar_borrows_leaf() {
        let node = Node::Scalar(EvalResult::String("x".to_string()));
        assert_eq!(node.as_scalar(), Some(&EvalResult::String("x".to_string())));
        assert_eq!(Node::Array(vec![]).as_scalar(), None);
    }
}
