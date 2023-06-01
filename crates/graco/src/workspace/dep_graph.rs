use petgraph::{
  data::{Element, FromElements},
  dot::{Config, Dot},
  graph::DiGraph,
  prelude::NodeIndex,
  visit::{DfsPostOrder, Walker},
};

use super::package::{Package, PackageIndex};

pub struct DepGraph {
  graph: DiGraph<String, ()>,
}

impl DepGraph {
  pub fn build(packages: &[Package]) -> Self {
    let edges = packages.iter().flat_map(|pkg| {
      pkg
        .all_dependencies()
        .filter_map(|name| packages.iter().find(|other_pkg| other_pkg.name == name))
        .map(move |dep| Element::Edge {
          source: pkg.index,
          target: dep.index,
          weight: (),
        })
    });

    let graph = DiGraph::<String, ()>::from_elements(
      packages
        .iter()
        .map(|pkg| Element::Node {
          weight: pkg.name.to_string(),
        })
        .chain(edges),
    );

    log::debug!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));

    DepGraph { graph }
  }

  pub fn is_dependent_on(&self, pkg: PackageIndex, dep: PackageIndex) -> bool {
    self.all_deps_for(pkg).any(|dep2| dep == dep2)
  }

  pub fn immediate_deps_for(&self, index: PackageIndex) -> impl Iterator<Item = PackageIndex> + '_ {
    self
      .graph
      .neighbors_directed(NodeIndex::new(index), petgraph::Direction::Outgoing)
      .map(|node| node.index())
  }

  pub fn all_deps_for(&self, index: PackageIndex) -> impl Iterator<Item = PackageIndex> + '_ {
    DfsPostOrder::new(&self.graph, NodeIndex::new(index))
      .iter(&self.graph)
      .map(|node| node.index())
      .filter(move |dep| *dep != index)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use maplit::hashset;
  use std::collections::HashSet;

  #[test]
  fn test_dep_graph() {
    let pkgs = crate::packages! [
      {"name": "a", "dependencies": {"b": "0.1.0"}},
      {"name": "b", "dependencies": {"c": "0.1.0"}},
      {"name": "c"}
    ];

    let dg = DepGraph::build(&pkgs);
    let deps_for = |n| dg.all_deps_for(n).collect::<HashSet<_>>();
    assert_eq!(deps_for(0), hashset! {1, 2});
    assert_eq!(deps_for(1), hashset! {2});
    assert_eq!(deps_for(2), hashset! {});

    let imm_deps_for = |n| dg.immediate_deps_for(n).collect::<HashSet<_>>();
    assert_eq!(imm_deps_for(0), hashset! {1});
    assert_eq!(imm_deps_for(1), hashset! {2});
    assert_eq!(imm_deps_for(2), hashset! {});

    assert!(dg.is_dependent_on(0, 1));
    assert!(dg.is_dependent_on(0, 2));
    assert!(!dg.is_dependent_on(1, 0));
  }
}
