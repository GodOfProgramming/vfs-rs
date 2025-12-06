use crate::{IdentKey, Relationship, Vfs};
use petgraph::graph::NodeIndex;
use smartstring::{Compact, SmartString};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VfsNode {
    pub(crate) ident: IdentKey,
    pub(crate) index: NodeIndex,

    pub(crate) name: SmartString<Compact>,
}

impl VfsNode {
    pub fn has_parent<T>(&self, vfs: &Vfs<T>) -> bool {
        vfs.inner
            .edges(self.index)
            .any(|e| matches!(e.weight(), Relationship::Parent(_)))
    }

    pub fn parent<'v, T>(&self, vfs: &'v Vfs<T>) -> Option<&'v Self> {
        vfs.inner.edges(self.index).find_map(|e| {
            if let Relationship::Parent(path) = e.weight() {
                Some(path)
            } else {
                None
            }
        })
    }

    pub fn lineage<'t, T>(&self, vfs: &'t Vfs<T>) -> Option<Vec<&'t str>> {
        std::iter::successors(Some(self), |p| p.parent(vfs))
            .map(|p| vfs.ident_of(p).map(|p| p.as_ref().as_str()))
            .collect::<Option<Vec<_>>>()
    }

    pub fn basename(&self) -> &str {
        self.name.as_str()
    }

    pub fn absolute<T>(&self, vfs: &Vfs<T>) -> Option<String> {
        let mut lineage = self.lineage(vfs)?;

        lineage.reverse();

        match lineage.len() {
            0 => None,
            1 => Some(lineage[0].to_string()),
            _ => Some(format!(
                "{}{}",
                lineage[0],
                itertools::join(&lineage[1..], "/")
            )),
        }
    }
}
