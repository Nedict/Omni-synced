//! A rectangular net of values: a surface's control points, a sample grid. Rectangular by
//! construction, so code that walks it never meets a missing cell.

/// `count[0]` values along u by `count[1]` along v; value `(i, j)` (i along u) is stored at
/// `j·count[0] + i`, u fastest.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid<T> {
    count: [usize; 2],
    cells: Vec<T>,
}

impl<T> Grid<T> {
    /// A grid from its cells, u fastest; `None` unless there are `count[0]·count[1]` of them.
    pub fn new(count: [usize; 2], cells: Vec<T>) -> Option<Grid<T>> {
        (count[0].checked_mul(count[1]) == Some(cells.len())).then_some(Grid { count, cells })
    }

    /// One row of values along u: `count` is `[cells.len(), 1]` (a curve's control points seen
    /// as a net).
    pub fn row(cells: Vec<T>) -> Grid<T> {
        Grid { count: [cells.len(), 1], cells }
    }

    /// A grid whose caller already holds `count[0]·count[1]` cells (a checked surface's net).
    pub(crate) fn checked(count: [usize; 2], cells: Vec<T>) -> Grid<T> {
        Grid { count, cells }
    }

    /// Values along u and along v.
    pub fn count(&self) -> [usize; 2] {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Value `(i, j)`, i along u.
    pub fn get(&self, i: usize, j: usize) -> Option<&T> {
        if i < self.count[0] { self.cells.get(j * self.count[0] + i) } else { None }
    }

    /// The values, u fastest.
    pub fn cells(&self) -> &[T] {
        &self.cells
    }

    /// The values to change in place; the grid keeps its shape.
    pub fn cells_mut(&mut self) -> &mut [T] {
        &mut self.cells
    }

    pub fn into_cells(self) -> Vec<T> {
        self.cells
    }

    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Grid<U> {
        Grid { count: self.count, cells: self.cells.into_iter().map(f).collect() }
    }

    /// [`Grid::map`] with a step that can fail: the first failure.
    pub fn try_map<U, E>(self, f: impl FnMut(T) -> Result<U, E>) -> Result<Grid<U>, E> {
        Ok(Grid { count: self.count, cells: self.cells.into_iter().map(f).collect::<Result<_, _>>()? })
    }

    /// Each value with the one in the same place of `other`; `None` unless their counts match.
    pub fn zip<U>(self, other: Grid<U>) -> Option<Grid<(T, U)>> {
        (self.count == other.count).then(|| Grid { count: self.count, cells: self.cells.into_iter().zip(other.cells).collect() })
    }

    /// The rows along v, one per u index: `rows[i][j]` is value `(i, j)` (truck's layout).
    pub fn into_rows(self) -> Vec<Vec<T>> {
        let cu = self.count[0];
        let mut rows: Vec<Vec<T>> = (0..cu).map(|_| Vec::with_capacity(self.count[1])).collect();
        for (k, v) in self.cells.into_iter().enumerate() {
            if let Some(row) = rows.get_mut(k % cu) {
                row.push(v);
            }
        }
        rows
    }
}

impl<'a, T> Grid<&'a T> {
    /// A grid over rows along v, one per u index (`rows[i][j]` is value `(i, j)`, truck's
    /// layout); `None` when the rows differ in length.
    pub fn from_rows<R: AsRef<[T]>>(rows: &'a [R]) -> Option<Grid<&'a T>> {
        let cv = rows.first().map_or(0, |r| r.as_ref().len());
        if rows.iter().any(|r| r.as_ref().len() != cv) {
            return None;
        }
        let cells = (0..cv).flat_map(|j| rows.iter().filter_map(move |r| r.as_ref().get(j))).collect();
        Grid::new([rows.len(), cv], cells)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_go_in_and_come_back_out() {
        // rows[i][j]: two along u, three along v.
        let rows = vec![vec![(0, 0), (0, 1), (0, 2)], vec![(1, 0), (1, 1), (1, 2)]];
        let g = Grid::from_rows(&rows).unwrap().map(|&v| v);
        assert_eq!(g.count(), [2, 3]);
        assert_eq!(g.cells(), &[(0, 0), (1, 0), (0, 1), (1, 1), (0, 2), (1, 2)]);
        assert_eq!(g.get(1, 2), Some(&(1, 2)));
        assert_eq!(g.get(2, 0), None);
        assert_eq!(g.into_rows(), rows);
        assert_eq!(Grid::row(vec![1, 2, 3]).count(), [3, 1]);
    }

    #[test]
    fn only_rectangles_are_grids() {
        assert!(Grid::from_rows(&[vec![1, 2], vec![3]]).is_none());
        assert!(Grid::new([2, 2], vec![1, 2, 3]).is_none());
        assert!(Grid::new([usize::MAX, 2], Vec::<u8>::new()).is_none());
        let empty: [Vec<u8>; 0] = [];
        assert!(Grid::from_rows(&empty).is_some_and(|g| g.is_empty()));
    }

    #[test]
    fn zip_needs_the_same_counts() {
        let a = Grid::new([2, 1], vec![1, 2]).unwrap();
        assert!(a.clone().zip(Grid::new([1, 2], vec![3, 4]).unwrap()).is_none());
        assert_eq!(a.zip(Grid::new([2, 1], vec!['a', 'b']).unwrap()).unwrap().cells(), &[(1, 'a'), (2, 'b')]);
        assert_eq!(Grid::new([1, 2], vec![1, -1]).unwrap().try_map(|x| if x > 0 { Ok(x) } else { Err(x) }), Err(-1));
    }
}
