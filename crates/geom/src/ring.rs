//! The edges of a closed ring of points: a polygon, a loop of segments, a fan of rays.

/// Each item with the next and the last with the first: the edges of the closed ring `pts`, as
/// many as there are items (a lone item pairs with itself; none come from an empty ring).
pub fn ring_edges<T>(pts: &[T]) -> impl Iterator<Item = (&T, &T)> + Clone {
    pts.iter().zip(pts.iter().cycle().skip(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_edge_closes_the_ring() {
        assert_eq!(ring_edges(&[1, 2, 3]).collect::<Vec<_>>(), vec![(&1, &2), (&2, &3), (&3, &1)]);
        assert_eq!(ring_edges(&[7]).collect::<Vec<_>>(), vec![(&7, &7)]);
        assert_eq!(ring_edges::<u8>(&[]).count(), 0);
    }
}
