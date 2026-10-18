//! A closed ring: a polygon's corners, a loop of segments, a fan of rays. The last item joins the
//! first, so code that walks the ring's edges never wraps an index by hand.

use std::ops::Deref;

use crate::Vec2;

/// Items in order around a closed ring; as a slice it is the items once each.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Ring<T>(Vec<T>);

impl<T> Ring<T> {
    pub fn new(items: Vec<T>) -> Ring<T> {
        Ring(items)
    }

    /// Each item with the next and the last with the first, as many edges as there are items (a
    /// lone item pairs with itself; an empty ring has none).
    pub fn edges(&self) -> impl Iterator<Item = (&T, &T)> + Clone {
        around(&self.0)
    }

    /// Each item with the one before and the one after it: `(previous, item, next)`, once per
    /// item.
    pub fn corners(&self) -> impl Iterator<Item = (&T, &T, &T)> + Clone {
        let before = self.0.iter().cycle().skip(self.0.len().saturating_sub(1));
        before.zip(self.edges()).map(|(p, (c, x))| (p, c, x))
    }

    pub fn into_vec(self) -> Vec<T> {
        self.0
    }
}

/// [`Ring::edges`] of items held elsewhere, for a type that keeps a closed ring in a field of its
/// own ([`crate::Loop2::joints`]).
pub(crate) fn around<T>(items: &[T]) -> impl Iterator<Item = (&T, &T)> + Clone {
    items.iter().zip(items.iter().cycle().skip(1))
}

impl<T> Deref for Ring<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.0
    }
}

impl<T> IntoIterator for Ring<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> std::vec::IntoIter<T> {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a Ring<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> std::slice::Iter<'a, T> {
        self.0.iter()
    }
}

impl<T> From<Vec<T>> for Ring<T> {
    fn from(items: Vec<T>) -> Ring<T> {
        Ring(items)
    }
}

impl<T, const N: usize> From<[T; N]> for Ring<T> {
    fn from(items: [T; N]) -> Ring<T> {
        Ring(items.into())
    }
}

impl<T> FromIterator<T> for Ring<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Ring<T> {
        Ring(items.into_iter().collect())
    }
}

impl Ring<Vec2> {
    /// The area the ring encloses, positive when it runs counter-clockwise.
    pub fn signed_area(&self) -> f64 {
        0.5 * self.edges().map(|(a, b)| a.cross(*b)).sum::<f64>()
    }

    /// Even-odd containment: whether a ray from `p` along +x crosses the ring an odd number of
    /// times.
    pub fn contains(&self, p: Vec2) -> bool {
        let mut inside = false;
        for (a, b) in self.edges() {
            if (a.y > p.y) != (b.y > p.y) {
                let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
                if p.x < x {
                    inside = !inside;
                }
            }
        }
        inside
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_edge_closes_the_ring() {
        assert_eq!(Ring::from([1, 2, 3]).edges().collect::<Vec<_>>(), vec![(&1, &2), (&2, &3), (&3, &1)]);
        assert_eq!(Ring::from([7]).edges().collect::<Vec<_>>(), vec![(&7, &7)]);
        assert_eq!(Ring::<u8>::default().edges().count(), 0);
        assert_eq!(Ring::from([1, 2, 3]).corners().collect::<Vec<_>>(), vec![(&3, &1, &2), (&1, &2, &3), (&2, &3, &1)]);
        assert_eq!(Ring::<u8>::default().corners().count(), 0);
    }

    #[test]
    fn a_square_encloses_its_area_and_its_inside() {
        let square = Ring::from([Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 2.0), Vec2::new(0.0, 2.0)]);
        assert_eq!(square.signed_area(), 4.0);
        assert_eq!(Ring::from_iter(square.iter().rev().copied()).signed_area(), -4.0);
        assert!(square.contains(Vec2::new(1.0, 1.0)));
        assert!(!square.contains(Vec2::new(3.0, 1.0)));
    }
}
