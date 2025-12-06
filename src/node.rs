use crate::{Relationship, Vfs};
use camino::Utf8PathBuf;
use petgraph::graph::NodeIndex;
use smartstring::{Compact, SmartString};
use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VfsNode {
    pub(crate) cached: SmartString<Compact>,
    pub(crate) name: SmartString<Compact>,
    pub(crate) inner: Utf8PathBuf,
    pub(crate) index: NodeIndex,
}

impl VfsNode {
    pub fn join(&self, name: impl AsRef<str>) -> Utf8PathBuf {
        self.inner.join(name.as_ref())
    }

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

    pub fn full_path(&self) -> &str {
        &self.cached
    }

    pub fn display(&self) -> &str {
        self.cached.as_str()
    }

    pub fn basename(&self) -> &str {
        self.name.as_str()
    }
}

impl PartialOrd for VfsNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VfsNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cached.cmp(&other.cached)
    }
}
