use crate::{IdentKey, Relationship, Vfs};
use petgraph::graph::NodeIndex;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VfsNode {
    pub(crate) ident: IdentKey,
    pub(crate) index: NodeIndex,
}

impl VfsNode {
    pub fn has_parent<T>(self, vfs: &Vfs<T>) -> bool {
        vfs.inner
            .edges(self.index)
            .any(|e| matches!(e.weight(), Relationship::Parent { .. }))
    }

    pub fn parent<T>(self, vfs: &Vfs<T>) -> Option<Self> {
        vfs.inner.edges(self.index).find_map(|e| {
            if let Relationship::Parent { node } = e.weight() {
                Some(*node)
            } else {
                None
            }
        })
    }

    pub fn lineage<'v, T>(self, vfs: &'v Vfs<T>) -> Option<Vec<&'v str>> {
        std::iter::successors(Some(self), |p| p.parent(vfs))
            .map(|p| vfs.ident_of(p).map(|p| p.as_ref().as_str()))
            .collect::<Option<Vec<_>>>()
    }

    pub fn basename<'v, T>(self, vfs: &'v Vfs<T>) -> Option<&'v str> {
        vfs.ident_of(self).map(|i| i.as_str())
    }

    pub fn absolute<T>(self, vfs: &Vfs<T>) -> Option<String> {
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

    pub fn iter<T>(self, vfs: &Vfs<T>) -> impl Iterator<Item = Self> {
        vfs.ls(self)
    }
}
