#[derive(Debug, Clone)]
pub enum QuadTree<T>
where
    T: Clone,
{
    Leaf(T),
    Node(T, Box<Children<T>>),
}

#[derive(Debug, Clone)]
pub struct Children<T>
where
    T: Clone,
{
    pub nw: QuadTree<T>,
    pub ne: QuadTree<T>,
    pub se: QuadTree<T>,
    pub sw: QuadTree<T>,

    depth_of_child: u32,
}

impl<T: Clone> QuadTree<T> {
    pub fn build_complete_tree(elements: Vec<T>, depth: u32) -> Self {
        let n = util::full_size(depth) as usize;
        assert_eq!(elements.len(), n);

        Self::build_node(&elements, 0, 0, depth)
    }

    fn build_node(all_elements: &[T], index: usize, level: u32, depth: u32) -> Self {
        if level == depth - 1 {
            return Self::Leaf(all_elements[index].clone());
        }

        let children_index = index << 2;
        Self::Node(
            all_elements[index].clone(),
            Box::new(Children {
                nw: Self::build_node(all_elements, children_index + 1, level + 1, depth),
                ne: Self::build_node(all_elements, children_index + 2, level + 1, depth),
                se: Self::build_node(all_elements, children_index + 3, level + 1, depth),
                sw: Self::build_node(all_elements, children_index + 4, level + 1, depth),
                depth_of_child: depth - level - 1,
            }),
        )
    }

    pub fn depth(&self) -> u32 {
        match self {
            QuadTree::Leaf(_) => 1,
            QuadTree::Node(_, q) => q.depth_of_child + 1,
        }
    }

    pub fn mut_view(&mut self) -> Vec<&mut T> {
        let mut flattened = Vec::with_capacity(util::full_size(self.depth()) as usize);
        match self {
            QuadTree::Leaf(v) => {
                flattened.push(v);
            }
            QuadTree::Node(e, q) => {
                flattened.push(e);
                let Children { nw, ne, se, sw, .. } = q.as_mut();
                for t in [nw, ne, se, sw] {
                    flattened.extend(t.mut_view());
                }
            }
        }
        flattened
    }

    pub fn items_at_level(&self, level: u32) -> Vec<&T> {
        let mut items = Vec::with_capacity(4usize.pow(level));

        match self {
            QuadTree::Leaf(e) => {
                if level != 0 {
                    panic!("not enough levels")
                }
                items.push(e)
            }

            QuadTree::Node(e, boxed_children) => {
                if level == 0 {
                    items.push(e)
                } else {
                    let Children { nw, ne, se, sw, .. } = &**boxed_children;
                    items.extend(
                        [nw, ne, se, sw]
                            .map(|q| q.items_at_level(level - 1))
                            .into_iter()
                            .flatten(),
                    )
                }
            }
        }

        items
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
    fn tree_makes_sense_three_levels() {
        let q = QuadTree::build_complete_tree((0..21).collect(), 3);
        println!("{q:?}");
    }

    #[test]
    fn at_level() {
        let q = QuadTree::build_complete_tree((0..21).collect(), 3);
        assert_eq!(q.items_at_level(0), vec![&0]);
        assert_eq!(q.items_at_level(1), vec![&1, &2, &3, &4]);
        assert_eq!(
            q.items_at_level(2)
                .into_iter()
                .map(|i| *i)
                .collect::<Vec<_>>(),
            (5..21).collect::<Vec<_>>()
        );
    }

    #[test]
    fn mut_view() {
        let mut q = QuadTree::build_complete_tree((0..21).collect(), 3);
        let view = q.mut_view();
        for i in view {
            *i += 1;
        }
        assert_eq!(dbg!(q).items_at_level(0), vec![&1])
    }
}
