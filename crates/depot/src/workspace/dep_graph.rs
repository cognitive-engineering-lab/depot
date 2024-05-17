use std::hash::Hash;

use bimap::BiHashMap;
use petgraph::{
  graph::DiGraph,
  prelude::NodeIndex,
  visit::{DfsPostOrder, Walker},
};

/// Generic data structure for representing dependencies between objects.
pub struct DepGraph<T> {
  graph: DiGraph<(), ()>,
  nodes: BiHashMap<T, NodeIndex>,
  roots: Vec<T>,
}

impl<T: Hash + PartialEq + Eq + Clone> DepGraph<T> {
  /// Creates a [`DepGraph`] starting with a set of `roots`, and then finds the dependencies
  /// by iteratively calling `compute_deps` for node.
  pub fn build(roots: Vec<T>, compute_deps: impl Fn(T) -> Vec<T>) -> Self {
    let mut graph = DiGraph::new();
    let mut nodes = BiHashMap::new();
    macro_rules! add_node {
      ($node:expr) => {
        match nodes.get_by_left($node) {
          Some(idx) => *idx,
          None => {
            let idx = graph.add_node(());
            nodes.insert($node.clone(), idx);
            idx
          }
        }
      };
    }
    let mut stack = vec![];

    for root in &roots {
      let idx = add_node!(root);
      stack.push((idx, root.clone()));
    }

    while let Some((idx, el)) = stack.pop() {
      for dep in compute_deps(el) {
        let dep_idx = add_node!(&dep);
        stack.push((dep_idx, dep));
        graph.add_edge(idx, dep_idx, ());
      }
    }

    DepGraph {
      roots,
      graph,
      nodes,
    }
  }

  fn index(&self, el: &T) -> NodeIndex {
    *self.nodes.get_by_left(el).unwrap()
  }

  fn value(&self, index: NodeIndex) -> &T {
    self.nodes.get_by_right(&index).unwrap()
  }
  
  pub fn nodes(&self) -> impl Iterator<Item = &T> {
    self.nodes.iter().map(|(node, _)| node)
  }

  pub fn is_dependent_on(&self, el: &T, dep: &T) -> bool {
    self.all_deps_for(el).any(|dep2| dep == dep2)
  }

  pub fn immediate_deps_for<'a>(&'a self, el: &T) -> impl Iterator<Item = &'a T> + 'a {
    self
      .graph
      .neighbors_directed(self.index(el), petgraph::Direction::Outgoing)
      .map(|node| self.value(node))
  }

  pub fn all_deps_for<'a>(&'a self, el: &T) -> impl Iterator<Item = &'a T> + 'a {
    let index = self.index(el);
    DfsPostOrder::new(&self.graph, index)
      .iter(&self.graph)
      .filter(move |dep| *dep != index)
      .map(|idx| self.value(idx))
  }

  pub fn roots(&self) -> impl Iterator<Item = &T> {
    self.roots.iter()
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use maplit::hashset;
  use std::collections::HashSet;

  #[test]
  fn dep_graph() {
    let dg = DepGraph::build(vec![0, 1], |i| match i {
      0 => vec![2],
      1 => vec![2],
      2 => vec![3],
      3 => vec![],
      _ => unreachable!(),
    });

    let z_idx = dg.index(&0);
    assert_eq!(*dg.value(z_idx), 0);

    assert_eq!(
      dg.nodes().copied().collect::<HashSet<_>>(),
      hashset! { 0, 1, 2, 3 }
    );

    assert!(dg.is_dependent_on(&0, &2));
    assert!(dg.is_dependent_on(&0, &3));
    assert!(!dg.is_dependent_on(&0, &1));

    assert_eq!(
      dg.immediate_deps_for(&0).copied().collect::<HashSet<_>>(),
      hashset! { 2 }
    );

    assert_eq!(
      dg.all_deps_for(&0).copied().collect::<HashSet<_>>(),
      hashset! { 2, 3 }
    );

    assert_eq!(
      dg.roots().copied().collect::<HashSet<_>>(),
      hashset! { 0, 1 }
    )
  }
}
