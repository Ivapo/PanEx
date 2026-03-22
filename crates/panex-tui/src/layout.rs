#[derive(Clone, PartialEq)]
pub enum SplitDirection {
    Vertical,   // left | right
    Horizontal, // top / bottom
}

#[derive(Clone)]
pub enum LayoutNode {
    Leaf {
        pane_id: String,
    },
    Split {
        direction: SplitDirection,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
        ratio: f64,
    },
}

pub fn count_leaves(node: &LayoutNode) -> usize {
    match node {
        LayoutNode::Leaf { .. } => 1,
        LayoutNode::Split { first, second, .. } => count_leaves(first) + count_leaves(second),
    }
}

pub fn collect_leaf_ids(node: &LayoutNode) -> Vec<String> {
    match node {
        LayoutNode::Leaf { pane_id } => vec![pane_id.clone()],
        LayoutNode::Split { first, second, .. } => {
            let mut ids = collect_leaf_ids(first);
            ids.extend(collect_leaf_ids(second));
            ids
        }
    }
}

pub fn split_pane(
    root: &LayoutNode,
    target_pane_id: &str,
    new_pane_id: &str,
    direction: SplitDirection,
) -> LayoutNode {
    match root {
        LayoutNode::Leaf { pane_id } => {
            if pane_id == target_pane_id {
                LayoutNode::Split {
                    direction,
                    first: Box::new(LayoutNode::Leaf {
                        pane_id: target_pane_id.to_string(),
                    }),
                    second: Box::new(LayoutNode::Leaf {
                        pane_id: new_pane_id.to_string(),
                    }),
                    ratio: 0.5,
                }
            } else {
                root.clone()
            }
        }
        LayoutNode::Split {
            direction: dir,
            first,
            second,
            ratio,
        } => LayoutNode::Split {
            direction: dir.clone(),
            first: Box::new(split_pane(first, target_pane_id, new_pane_id, direction.clone())),
            second: Box::new(split_pane(second, target_pane_id, new_pane_id, direction)),
            ratio: *ratio,
        },
    }
}

pub fn remove_pane(root: &LayoutNode, pane_id: &str) -> Option<LayoutNode> {
    match root {
        LayoutNode::Leaf { pane_id: id } => {
            if id == pane_id {
                None
            } else {
                Some(root.clone())
            }
        }
        LayoutNode::Split {
            direction,
            first,
            second,
            ratio,
        } => {
            let first_result = remove_pane(first, pane_id);
            let second_result = remove_pane(second, pane_id);

            match (first_result, second_result) {
                (None, second) => second,
                (first, None) => first,
                (Some(f), Some(s)) => Some(LayoutNode::Split {
                    direction: direction.clone(),
                    first: Box::new(f),
                    second: Box::new(s),
                    ratio: *ratio,
                }),
            }
        }
    }
}
