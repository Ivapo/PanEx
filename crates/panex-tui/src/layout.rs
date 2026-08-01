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

/// Ratio given to the side of a split holding a pane at each size level.
/// A split starts even at 0.5; ±25% of that is 0.625 / 0.375.
pub fn ratio_for_level(level: i8) -> f64 {
    match level {
        l if l > 0 => 0.625,
        l if l < 0 => 0.375,
        _ => 0.5,
    }
}

/// One step on the root→leaf path: the split's axis and the side taken.
struct Step {
    direction: SplitDirection,
    took_first: bool,
}

fn path_to_pane(node: &LayoutNode, pane_id: &str) -> Option<Vec<Step>> {
    match node {
        LayoutNode::Leaf { pane_id: id } => (id == pane_id).then(Vec::new),
        LayoutNode::Split {
            direction,
            first,
            second,
            ..
        } => {
            let (mut steps, took_first) = match path_to_pane(first, pane_id) {
                Some(s) => (s, true),
                None => (path_to_pane(second, pane_id)?, false),
            };
            steps.insert(
                0,
                Step {
                    direction: direction.clone(),
                    took_first,
                },
            );
            Some(steps)
        }
    }
}

/// Leaves whose size on `direction`'s axis is set by the boundary *above*
/// `node`. Descent stops at any split on the same axis — those leaves answer
/// to a nearer boundary of their own, so the outer one doesn't speak for them.
fn leaves_bounded_by(node: &LayoutNode, direction: &SplitDirection) -> Vec<String> {
    match node {
        LayoutNode::Leaf { pane_id } => vec![pane_id.clone()],
        LayoutNode::Split {
            direction: dir,
            first,
            second,
            ..
        } => {
            if dir == direction {
                Vec::new()
            } else {
                let mut ids = leaves_bounded_by(first, direction);
                ids.extend(leaves_bounded_by(second, direction));
                ids
            }
        }
    }
}

/// Resize `pane_id` on one axis by moving its nearest ancestor split of
/// `direction` to `level`.
///
/// Returns every pane sharing that boundary — including `pane_id` itself.
/// Their previous claim on this axis is void, so the caller resets them.
/// Returns `None` when the pane has no ancestor on this axis, i.e. there is
/// no split in that direction and nothing to redistribute.
pub fn resize_axis(
    root: &mut LayoutNode,
    pane_id: &str,
    direction: SplitDirection,
    level: i8,
) -> Option<Vec<String>> {
    let path = path_to_pane(root, pane_id)?;
    // Nearest ancestor on this axis = deepest matching split on the path.
    let target = path.iter().rposition(|s| s.direction == direction)?;

    let mut node: &mut LayoutNode = root;
    for step in &path[..target] {
        node = match node {
            LayoutNode::Split { first, second, .. } => {
                if step.took_first {
                    first.as_mut()
                } else {
                    second.as_mut()
                }
            }
            LayoutNode::Leaf { .. } => return None,
        };
    }

    let LayoutNode::Split {
        first,
        second,
        ratio,
        ..
    } = node
    else {
        return None;
    };

    let grown = ratio_for_level(level);
    *ratio = if path[target].took_first {
        grown
    } else {
        1.0 - grown
    };

    let mut affected = leaves_bounded_by(first, &direction);
    affected.extend(leaves_bounded_by(second, &direction));
    Some(affected)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: &str) -> LayoutNode {
        LayoutNode::Leaf {
            pane_id: id.to_string(),
        }
    }

    fn split(direction: SplitDirection, first: LayoutNode, second: LayoutNode) -> LayoutNode {
        LayoutNode::Split {
            direction,
            first: Box::new(first),
            second: Box::new(second),
            ratio: 0.5,
        }
    }

    fn ratio_at(node: &LayoutNode) -> f64 {
        match node {
            LayoutNode::Split { ratio, .. } => *ratio,
            LayoutNode::Leaf { .. } => panic!("not a split"),
        }
    }

    fn child(node: &LayoutNode, first: bool) -> &LayoutNode {
        match node {
            LayoutNode::Split {
                first: f, second: s, ..
            } => {
                if first {
                    f
                } else {
                    s
                }
            }
            LayoutNode::Leaf { .. } => panic!("not a split"),
        }
    }

    /// A lone pane fills the screen — there is no boundary to move.
    #[test]
    fn single_pane_has_no_axis_to_resize() {
        let mut root = leaf("a");
        assert!(resize_axis(&mut root, "a", SplitDirection::Vertical, 1).is_none());
        assert!(resize_axis(&mut root, "a", SplitDirection::Horizontal, 1).is_none());
    }

    /// Growing the left pane pushes the divider right; growing the right one
    /// pushes it left. Same 25%, mirrored.
    #[test]
    fn side_determines_direction_of_growth() {
        let mut root = split(SplitDirection::Vertical, leaf("a"), leaf("b"));
        resize_axis(&mut root, "a", SplitDirection::Vertical, 1).unwrap();
        assert_eq!(ratio_at(&root), 0.625);

        resize_axis(&mut root, "b", SplitDirection::Vertical, 1).unwrap();
        assert_eq!(ratio_at(&root), 0.375);

        resize_axis(&mut root, "a", SplitDirection::Vertical, -1).unwrap();
        assert_eq!(ratio_at(&root), 0.375);

        resize_axis(&mut root, "a", SplitDirection::Vertical, 0).unwrap();
        assert_eq!(ratio_at(&root), 0.5);
    }

    /// A left/right split gives no height to redistribute, so `+` only widens.
    #[test]
    fn axis_without_a_split_does_not_move() {
        let mut root = split(SplitDirection::Vertical, leaf("a"), leaf("b"));
        assert!(resize_axis(&mut root, "a", SplitDirection::Horizontal, 1).is_none());
        assert_eq!(ratio_at(&root), 0.5);
    }

    /// In a 2x2 grid, growing `a` widens its whole column (a and c share it)
    /// and makes `a` taller within that column only.
    #[test]
    fn grid_growth_moves_the_nearest_boundary_on_each_axis() {
        let mut root = split(
            SplitDirection::Vertical,
            split(SplitDirection::Horizontal, leaf("a"), leaf("c")),
            split(SplitDirection::Horizontal, leaf("b"), leaf("d")),
        );

        let widened = resize_axis(&mut root, "a", SplitDirection::Vertical, 1).unwrap();
        assert_eq!(ratio_at(&root), 0.625);
        // Every pane's width answers to the root boundary.
        assert_eq!(widened, vec!["a", "c", "b", "d"]);

        let heightened = resize_axis(&mut root, "a", SplitDirection::Horizontal, 1).unwrap();
        assert_eq!(ratio_at(child(&root, true)), 0.625);
        // ...but only the left column's height does.
        assert_eq!(heightened, vec!["a", "c"]);
        assert_eq!(ratio_at(child(&root, false)), 0.5, "right column untouched");
    }

    /// Nested same-axis splits: `b` answers to the inner divider, not the root,
    /// and panes behind their own inner divider keep their claim.
    #[test]
    fn nearest_ancestor_wins_over_outer_one() {
        let mut root = split(
            SplitDirection::Vertical,
            leaf("a"),
            split(SplitDirection::Vertical, leaf("b"), leaf("c")),
        );

        let affected = resize_axis(&mut root, "b", SplitDirection::Vertical, 1).unwrap();
        assert_eq!(ratio_at(&root), 0.5, "outer divider stays put");
        assert_eq!(ratio_at(child(&root, false)), 0.625);
        assert_eq!(affected, vec!["b", "c"]);

        // Growing `a` moves the root divider, which does not speak for b/c —
        // they sit behind an inner divider of their own.
        let affected = resize_axis(&mut root, "a", SplitDirection::Vertical, 1).unwrap();
        assert_eq!(ratio_at(&root), 0.625);
        assert_eq!(affected, vec!["a"]);
    }

    #[test]
    fn unknown_pane_is_a_no_op() {
        let mut root = split(SplitDirection::Vertical, leaf("a"), leaf("b"));
        assert!(resize_axis(&mut root, "zzz", SplitDirection::Vertical, 1).is_none());
        assert_eq!(ratio_at(&root), 0.5);
    }
}
