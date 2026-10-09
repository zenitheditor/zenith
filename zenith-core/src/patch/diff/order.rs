//! Which paired children keep their place when a sibling list reorders.

/// Marks the after children that stay in place: a longest run of paired
/// children whose before indices increase. `paired[j]` is the before index
/// of after child `j`, `None` for a new child. Every other paired child
/// moves.
///
/// On a tie the later after child stays. A swap of two neighbours moves the
/// earlier one, which gives the same text as moving the later one.
pub(super) fn kept_in_place(paired: &[Option<usize>]) -> Vec<bool> {
    // `tails[k]` is the after index that ends the best run of length `k + 1`.
    let mut tails: Vec<usize> = Vec::new();
    // `prev[j]` is the after index before `j` in its best run.
    let mut prev: Vec<Option<usize>> = vec![None; paired.len()];
    for (j, value) in paired.iter().enumerate() {
        let Some(v) = *value else {
            continue;
        };
        let pos = tails.partition_point(|&t| paired.get(t).copied().flatten() < Some(v));
        if let Some(slot) = prev.get_mut(j) {
            *slot = pos.checked_sub(1).and_then(|p| tails.get(p).copied());
        }
        if pos == tails.len() {
            tails.push(j);
        } else if let Some(slot) = tails.get_mut(pos) {
            *slot = j;
        }
    }
    let mut kept = vec![false; paired.len()];
    let mut cursor = tails.last().copied();
    while let Some(j) = cursor {
        if let Some(slot) = kept.get_mut(j) {
            *slot = true;
        }
        cursor = prev.get(j).copied().flatten();
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moved(paired: &[Option<usize>]) -> Vec<usize> {
        kept_in_place(paired)
            .iter()
            .enumerate()
            .filter(|(j, k)| !**k && paired[*j].is_some())
            .map(|(j, _)| j)
            .collect()
    }

    #[test]
    fn in_order_list_keeps_everything() {
        assert!(moved(&[Some(0), None, Some(1), Some(2)]).is_empty());
    }

    #[test]
    fn neighbour_swap_moves_one_node() {
        // [a, x, b] -> [a, b, x]: b moves in front of x.
        assert_eq!(moved(&[Some(0), Some(2), Some(1)]), vec![1]);
    }

    #[test]
    fn move_to_front_moves_only_the_first_node() {
        // [t, a, b, c] -> [a, b, c, t].
        assert_eq!(moved(&[Some(1), Some(2), Some(3), Some(0)]), vec![3]);
    }

    #[test]
    fn move_to_back_moves_only_the_last_node() {
        // [a, b, c, t] -> [t, a, b, c].
        assert_eq!(moved(&[Some(3), Some(0), Some(1), Some(2)]), vec![0]);
    }

    #[test]
    fn empty_list() {
        assert!(kept_in_place(&[]).is_empty());
    }
}
