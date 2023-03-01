use super::atom::{C, H};
use num_traits::Num;
use petgraph::{
    algo::is_isomorphic_subgraph_matching,
    graph::{EdgeIndex, NodeIndex},
    prelude::Graph,
    Undirected,
};
use std::ops::{Deref, DerefMut};

// type Index = NodeIndex<DefaultIx>;

/// Molecule
#[derive(Clone, Debug, Default)]
pub struct Molecule<N = f64, E = u8>(Graph<N, E, Undirected>);

impl<E: Num> Molecule<f64, E> {
    pub fn new() -> Self {
        Molecule(Graph::new_undirected())
    }

    pub fn h(&self) -> impl Iterator<Item = NodeIndex> + '_ {
        self.node_indices().filter(move |&index| self[index] == H)
    }

    pub fn c(&self) -> impl Iterator<Item = NodeIndex> + '_ {
        self.node_indices().filter(move |&index| self[index] == C)
    }

    pub fn is_isomorphic(&self, other: &Self) -> bool {
        is_isomorphic_subgraph_matching(&self.0, &other.0, PartialEq::eq, PartialEq::eq)
    }
}

impl<N, E> Deref for Molecule<N, E> {
    type Target = Graph<N, E, Undirected>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<N, E> DerefMut for Molecule<N, E> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

mod alkanes {
    use super::{Molecule, C, H};
    use std::array::from_fn;

    // Methane (CH4)
    pub fn methane() -> Molecule {
        let mut molecule = Molecule::new();
        let c: [_; 1] = from_fn(|_| molecule.add_node(C));
        let h: [_; 4] = from_fn(|_| molecule.add_node(H));
        molecule.extend_with_edges(&[
            (h[0], c[0], 1),
            (h[1], c[0], 1),
            (h[2], c[0], 1),
            (h[3], c[0], 1),
        ]);
        molecule
    }

    // Ethane (C2H6)
    pub fn ethane() -> Molecule {
        let mut molecule = Molecule::new();
        let c: [_; 2] = from_fn(|_| molecule.add_node(C));
        let h: [_; 6] = from_fn(|_| molecule.add_node(H));
        molecule.extend_with_edges(&[
            (c[0], c[1], 1),
            (h[0], c[0], 1),
            (h[1], c[0], 1),
            (h[2], c[0], 1),
            (h[3], c[1], 1),
            (h[4], c[1], 1),
            (h[5], c[1], 1),
        ]);
        molecule
    }

    // Propane (C3H8)
    pub fn propane() -> Molecule {
        let mut molecule = Molecule::new();
        let c: [_; 3] = from_fn(|_| molecule.add_node(C));
        let h: [_; 8] = from_fn(|_| molecule.add_node(H));
        molecule.extend_with_edges(&[
            (c[0], c[1], 1),
            (c[1], c[2], 1),
            (h[0], c[0], 1),
            (h[1], c[0], 1),
            (h[2], c[0], 1),
            (h[3], c[1], 1),
            (h[4], c[1], 1),
            (h[5], c[2], 1),
            (h[6], c[2], 1),
            (h[7], c[2], 1),
        ]);
        molecule
    }

    // Butane
    // Pentane
    // Hexane
    // Heptane
    // Octane
    // Nonane
    // Decane
}

mod alkenes {
    use super::{Molecule, C, H};
    use std::array::from_fn;

    // Ethene (C₂H₄
    pub fn ethene() -> Molecule {
        let mut molecule = Molecule::new();
        let c: [_; 2] = from_fn(|_| molecule.add_node(C));
        let h: [_; 4] = from_fn(|_| molecule.add_node(H));
        molecule.extend_with_edges(&[
            (c[0], c[1], 2),
            (h[0], c[0], 1),
            (h[1], c[0], 1),
            (h[2], c[1], 1),
            (h[3], c[1], 1),
        ]);
        molecule
    }

    // Propene (C₃H₆
    pub fn propene() -> Molecule {
        let mut molecule = Molecule::new();
        let c: [_; 3] = from_fn(|_| molecule.add_node(C));
        let h: [_; 6] = from_fn(|_| molecule.add_node(H));
        molecule.extend_with_edges(&[
            (c[0], c[1], 2),
            (c[1], c[2], 1),
            (h[0], c[0], 1),
            (h[1], c[0], 1),
            (h[2], c[1], 1),
            (h[3], c[2], 1),
            (h[4], c[2], 1),
            (h[5], c[2], 1),
        ]);
        molecule
    }

    // Butene (C₄H₈
    pub fn butene() -> Molecule {
        let mut molecule = Molecule::new();
        let c: [_; 4] = from_fn(|_| molecule.add_node(C));
        let h: [_; 8] = from_fn(|_| molecule.add_node(H));
        molecule.extend_with_edges(&[
            (c[0], c[1], 2),
            (c[1], c[2], 1),
            (c[2], c[3], 1),
            (h[0], c[0], 1),
            (h[1], c[0], 1),
            (h[2], c[1], 1),
            (h[3], c[2], 1),
            (h[4], c[2], 1),
            (h[5], c[3], 1),
            (h[6], c[3], 1),
            (h[7], c[3], 1),
        ]);
        molecule
    }

    // Pentene (C₅H₁₀)
    pub fn pentene() -> Molecule {
        let mut molecule = Molecule::new();
        let c: [_; 5] = from_fn(|_| molecule.add_node(C));
        let h: [_; 10] = from_fn(|_| molecule.add_node(H));
        molecule.extend_with_edges(&[
            (c[0], c[1], 2),
            (c[1], c[2], 1),
            (c[2], c[3], 1),
            (c[3], c[4], 1),
            (h[0], c[0], 1),
            (h[1], c[0], 1),
            (h[2], c[1], 1),
            (h[3], c[2], 1),
            (h[4], c[2], 1),
            (h[5], c[3], 1),
            (h[6], c[3], 1),
            (h[7], c[4], 1),
            (h[8], c[4], 1),
            (h[9], c[4], 1),
        ]);
        molecule
    }

    // Hexene (C₆H₁₂
    // Heptene (C₇H₁₄
    // Octene (C₈H₁₆
    // Nonene (C₉H₁₈
    // Decene (C₁₀H₂₀

    #[test]
    fn bounds() {
        for molecule in [ethene(), propene(), butene(), pentene()] {
            assert_eq!(
                molecule
                    .edge_indices()
                    .filter(|&bound| {
                        molecule.edge_endpoints(bound).is_some_and(|(left, right)| {
                            molecule[bound] == 2 && molecule[left] == C && molecule[right] == C
                        })
                    })
                    .count(),
                1
            );
        }
    }
}

#[cfg(test)]
mod test {
    use super::{
        alkenes::{butene, ethene, pentene, propene},
        Molecule, C, H,
    };
    use std::array::from_fn;

    #[test]
    fn bounds() {
        for molecule in [ethene(), propene(), butene(), pentene()] {
            assert!(molecule
                .c()
                .all(|c| molecule.edges(c).map(|edge| edge.weight()).sum::<u8>() == 4));
            assert!(molecule
                .h()
                .all(|h| molecule.edges(h).map(|edge| edge.weight()).sum::<u8>() == 1));
        }
    }

    #[test]
    fn test() {
        let mut molecule = Molecule::new();
        let c: [_; 1] = from_fn(|_| molecule.add_node(C));
        let h: [_; 1] = from_fn(|_| molecule.add_node(H));
        molecule.extend_with_edges(&[
            // (c[0], h[0], 1),
            (h[0], c[0], 1),
            // (h[1], c[0], 1),
            // (h[2], c[0], 1),
            // (h[3], c[1], 1),
            // (h[4], c[1], 1),
        ]);
        println!("{molecule:?}");
        println!("{:?}", propene());
        let check = molecule.is_isomorphic(&propene());
        println!("{check}");
    }
}
