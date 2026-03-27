use std::cmp::Ordering;
use std::collections::BinaryHeap;
use tree_sitter::Tree;

/// An entry in the reduction queue, representing a named node to try reducing.
/// Stores stable identifiers (byte ranges, not Node handles) since nodes are
/// invalidated when the tree is reparsed.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct QueueEntry {
    /// Start byte of the node in the source.
    pub start_byte: usize,
    /// End byte of the node in the source.
    pub end_byte: usize,
    /// The node's kind ID (grammar symbol).
    pub kind_id: u16,
    /// Number of descendant leaf nodes (both named and anonymous).
    /// Used for priority ordering — larger nodes are tried first.
    pub token_count: usize,
}

impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Larger token_count = higher priority
        self.token_count
            .cmp(&other.token_count)
            // Break ties by start_byte (earlier = higher priority)
            .then_with(|| other.start_byte.cmp(&self.start_byte))
    }
}

impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Priority queue for the Perses-style reduction algorithm.
/// Nodes are ordered by token count (largest first).
pub struct ReductionQueue {
    heap: BinaryHeap<QueueEntry>,
}

impl ReductionQueue {
    /// Build a queue from all named nodes in the tree.
    pub fn from_tree(tree: &Tree) -> Self {
        let mut heap = BinaryHeap::new();
        let root = tree.root_node();
        let mut cursor = root.walk();
        collect_named_nodes(&mut cursor, &mut heap);
        Self { heap }
    }

    /// Rebuild the queue from a new tree (after a reduction was accepted).
    pub fn rebuild(&mut self, tree: &Tree) {
        self.heap.clear();
        let root = tree.root_node();
        let mut cursor = root.walk();
        collect_named_nodes(&mut cursor, &mut self.heap);
    }

    /// Pop the highest-priority entry (largest token count).
    pub fn pop(&mut self) -> Option<QueueEntry> {
        self.heap.pop()
    }

    /// Number of entries remaining.
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}

/// Count leaf nodes (nodes with no children) in the subtree.
fn count_leaves(node: &tree_sitter::Node) -> usize {
    if node.child_count() == 0 {
        return 1;
    }
    let mut count = 0;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            count += count_leaves(&cursor.node());
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    count
}

fn collect_named_nodes(cursor: &mut tree_sitter::TreeCursor, heap: &mut BinaryHeap<QueueEntry>) {
    let node = cursor.node();
    if node.is_named() {
        heap.push(QueueEntry {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            kind_id: node.grammar_id(),
            token_count: count_leaves(&node),
        });
    }
    if cursor.goto_first_child() {
        loop {
            collect_named_nodes(cursor, heap);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tree(source: &[u8]) -> Tree {
        let lang = bonsai_core::languages::get_language("python").unwrap();
        bonsai_core::parse::parse(source, &lang).unwrap()
    }

    #[test]
    fn test_queue_pops_largest_first() {
        let tree = make_tree(b"x = 1\ny = 2 + 3\nz = 4");
        let mut queue = ReductionQueue::from_tree(&tree);

        let mut prev_count = usize::MAX;
        while let Some(entry) = queue.pop() {
            // Each entry should have equal or smaller token count than previous
            assert!(
                entry.token_count <= prev_count,
                "Queue should pop in descending token count order: got {} after {}",
                entry.token_count,
                prev_count
            );
            prev_count = entry.token_count;
        }
    }

    #[test]
    fn test_queue_entries_are_byte_ranges() {
        let source = b"x = 1";
        let tree = make_tree(source);
        let mut queue = ReductionQueue::from_tree(&tree);

        while let Some(entry) = queue.pop() {
            // All entries should have valid byte ranges within source
            assert!(entry.start_byte <= entry.end_byte);
            assert!(entry.end_byte <= source.len());
        }
    }

    #[test]
    fn test_queue_only_named_nodes() {
        let tree = make_tree(b"x = 1");
        let queue = ReductionQueue::from_tree(&tree);

        // Should have at least: module, expression_statement, assignment, identifier, integer
        assert!(
            queue.len() >= 3,
            "Should have multiple named nodes, got {}",
            queue.len()
        );
    }

    #[test]
    fn test_queue_rebuild_after_modification() {
        let source1 = b"x = 1\ny = 2\nz = 3";
        let tree1 = make_tree(source1);
        let mut queue = ReductionQueue::from_tree(&tree1);
        let original_len = queue.len();

        // "Reduce" by parsing a shorter source
        let source2 = b"x = 1";
        let tree2 = make_tree(source2);
        queue.rebuild(&tree2);

        assert!(
            queue.len() < original_len,
            "Rebuilt queue should have fewer entries: {} vs {}",
            queue.len(),
            original_len
        );
    }

    #[test]
    fn test_queue_empty_source() {
        let tree = make_tree(b"");
        let queue = ReductionQueue::from_tree(&tree);
        // Empty Python file has a "module" node
        assert!(queue.len() <= 1);
    }

    #[test]
    fn test_queue_pop_returns_none_when_empty() {
        let tree = make_tree(b"");
        let mut queue = ReductionQueue::from_tree(&tree);
        // Drain all entries
        while queue.pop().is_some() {}
        assert!(queue.pop().is_none());
        assert!(queue.is_empty());
    }
}
