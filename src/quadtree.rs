use std::ops::Index;

#[derive(Debug)]
pub enum QuadTree<T> {
    Element(T),
    Quadrant(Box<Quadrant<T>>),
}

#[derive(Debug)]
pub struct Quadrant<T> {
    pub nw: QuadTree<T>,
    pub ne: QuadTree<T>,
    pub se: QuadTree<T>,
    pub sw: QuadTree<T>,

    depth: u32,
}

impl<T> QuadTree<T> {
    pub fn build_tree(mut elements: Vec<T>, depth: u32) -> QuadTree<T> {
        let n = 4usize.pow(depth);

        assert_eq!(elements.len(), n);

        if depth == 0 {
            let elem = elements.pop();
            return QuadTree::Element(elem.unwrap());
        }

        let mut south = elements.split_off(n / 2);
        let mut north = elements;

        let ne = north.split_off(n / 4);
        let nw = north;

        let sw = south.split_off(n / 4);
        let se = south;

        QuadTree::Quadrant(Box::new(Quadrant {
            nw: Self::build_tree(nw, depth - 1),
            ne: Self::build_tree(ne, depth - 1),
            se: Self::build_tree(se, depth - 1),
            sw: Self::build_tree(sw, depth - 1),
            depth,
        }))
    }

    pub fn depth(&self) -> u32 {
        match self {
            QuadTree::Element(_) => 0,
            QuadTree::Quadrant(q) => q.depth,
        }
    }

    pub fn mut_view(&mut self) -> Vec<&mut T> {
        let mut flattened = Vec::with_capacity(util::full_size(self.depth()) as usize);
        match self {
            QuadTree::Element(v) => {
                flattened.push(v);
            }
            QuadTree::Quadrant(q) => {
                let Quadrant { nw, ne, se, sw, .. } = q.as_mut();
                for t in [nw, ne, se, sw] {
                    flattened.extend(t.mut_view());
                }
            }
        }
        flattened
    }
}

impl<T> Index<usize> for QuadTree<T> {
    type Output = T;

    fn index(&self, flat_id: usize) -> &Self::Output {
        match self {
            QuadTree::Element(v) => {
                if flat_id != 0 {
                    panic!("ID out of bound");
                }
                v
            }

            QuadTree::Quadrant(boxed_quadrant) => {
                let Quadrant {
                    nw,
                    ne,
                    se,
                    sw,
                    depth,
                } = &**boxed_quadrant;

                let n = 4usize.pow(*depth - 1);

                if flat_id < n {
                    &nw[flat_id]
                } else if flat_id >= n && flat_id < 2 * n {
                    &ne[flat_id - n]
                } else if flat_id >= 2 * n && flat_id < 3 * n {
                    &se[flat_id - 2 * n]
                } else if flat_id >= 3 * n && flat_id < 4 * n {
                    &sw[flat_id - 3 * n]
                } else {
                    panic!("ID out of bound")
                }
            }
        }
    }
}

impl<T> Index<(usize, usize)> for QuadTree<T> {
    type Output = T;

    fn index(&self, index: (usize, usize)) -> &Self::Output {
        match self {
            QuadTree::Element(_) => {
                panic!("Cannot index into element")
            }

            QuadTree::Quadrant(boxed_quadrant) => {
                let Quadrant {
                    nw,
                    ne,
                    se,
                    sw,
                    depth,
                } = &**boxed_quadrant;

                if depth == &1 {
                    &(match index {
                        (0, 0) => nw,
                        (0, 1) => ne,
                        (1, 1) => se,
                        (1, 0) => sw,
                        _ => panic!("Index out of bounds"),
                    })[0]
                } else {
                    let half_width = 2usize.pow(*depth - 1);

                    if index.0 < half_width && index.1 < half_width {
                        &nw[index]
                    } else if index.0 < half_width && index.1 >= half_width {
                        &ne[(index.0, index.1 - half_width)]
                    } else if index.0 >= half_width && index.1 >= half_width {
                        &se[(index.0 - half_width, index.1 - half_width)]
                    } else {
                        &sw[(index.0 - half_width, index.1)]
                    }
                }
            }
        }
    }
}

pub mod util {
    pub fn full_size(depth: u32) -> u32 {
        4u32.pow(depth) / 3
    }

    pub fn node_index(level: u32, row: u32, col: u32) -> u32 {
        full_size(level) + (row << level) + col
    }
}

#[cfg(test)]
mod test {
    use super::QuadTree;

    #[test]
    fn one_level_indexing() {
        let q = QuadTree::build_tree((0..4).collect(), 1);
        assert_eq!(q[(0, 0)], 0);
        assert_eq!(q[(0, 1)], 1);
        assert_eq!(q[(1, 1)], 2);
        assert_eq!(q[(1, 0)], 3);
    }

    #[test]
    fn two_level_indexing() {
        let cycle = vec![0, 1, 5, 4, 2, 3, 7, 6, 10, 11, 15, 14, 8, 9, 13, 12];

        let q = QuadTree::build_tree(cycle, 2);

        for row in 0..4 {
            for col in 0..4 {
                assert_eq!(q[(row, col)], row * 4 + col);
            }
        }
    }

    #[test]
    fn one_level_atting() {
        let q = QuadTree::build_tree((0..4).collect(), 1);
        assert_eq!(q[0], 0);
        assert_eq!(q[1], 1);
        assert_eq!(q[2], 2);
        assert_eq!(q[3], 3);
    }

    #[test]
    fn two_level_atting() {
        let q = QuadTree::build_tree((0..16).collect(), 2);
        for i in 0..16 {
            assert_eq!(q[i], i);
        }
    }

    #[test]
    fn up_to_10_level_atting() {
        for depth in 1..=10 {
            let q = QuadTree::build_tree((0..(4usize.pow(depth))).collect(), depth);
            for i in 0..(4usize.pow(depth)) {
                assert_eq!(q[i], i);
            }
        }
    }

    #[test]
    fn flattened_view_one_level() {
        let mut t = QuadTree::build_tree((0..4).collect(), 1);
        assert_eq!(t.mut_view(), vec![&mut 0, &mut 1, &mut 2, &mut 3])
    }

    #[test]
    fn flattened_view_two_level() {
        let mut t = QuadTree::build_tree((0..16).collect(), 2);
        assert_eq!(
            t.mut_view(),
            (0..16)
                .collect::<Vec::<i32>>()
                .iter_mut()
                .collect::<Vec::<&mut i32>>()
        )
    }
}
