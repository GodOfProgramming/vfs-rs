pub mod entry;
pub mod error;
pub mod node;

pub use crate::{
    entry::VfsEntry,
    error::{VfsError, VfsResult},
    node::VfsNode,
};
use camino::Utf8PathBuf;
use itertools::{FoldWhile, Itertools};
use petgraph::{Graph, visit::EdgeRef};
use smartstring::{Compact, SmartString};
use std::{borrow::Borrow, fmt::Debug, hash::Hash, ops::Deref};

pub struct Vfs<T> {
    inner: Graph<VfsEntry<T>, Relationship>,
    root: VfsNode,
}

impl<T> Default for Vfs<T> {
    fn default() -> Self {
        let mut graph = Graph::new();
        let root_index = graph.add_node(VfsEntry::Dir);
        Self {
            inner: graph,
            root: VfsNode {
                cached: SmartString::from("/"),
                name: SmartString::from("/"),
                inner: Utf8PathBuf::from("/"),
                index: root_index,
            },
        }
    }
}

impl<T> Vfs<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn root(&self) -> &VfsNode {
        &self.root
    }

    pub fn find(&self, path: impl AsRef<str>) -> Option<&VfsNode> {
        self.inner.edge_weights().find_map(|edge| match edge {
            Relationship::Parent(_) => None,
            Relationship::Child(vfs_path) => {
                if vfs_path.cached == path.as_ref() {
                    Some(vfs_path)
                } else {
                    None
                }
            }
        })
    }

    pub fn ls(&self, path: impl Borrow<VfsNode>) -> impl Iterator<Item = &VfsNode> {
        self.inner.edges(path.borrow().index).filter_map(|e| {
            if let Relationship::Child(dir) = e.weight() {
                Some(dir)
            } else {
                None
            }
        })
    }

    pub fn lookup(
        &self,
        path: impl Borrow<VfsNode>,
        name: impl AsRef<str>,
    ) -> Option<(&VfsNode, &VfsEntry<T>)> {
        self.inner.edges(path.borrow().index).find_map(|e| {
            if e.weight().name == name.as_ref() {
                Some(e.weight().deref()).zip(self.inner.node_weight(e.target()))
            } else {
                None
            }
        })
    }

    pub fn lookup_path(
        &self,
        path: impl Borrow<VfsNode>,
        name: impl AsRef<str>,
    ) -> Option<&VfsNode> {
        self.lookup(path, name.as_ref()).map(|(p, _)| p)
    }

    pub fn lookup_node(
        &self,
        path: impl Borrow<VfsNode>,
        name: impl AsRef<str>,
    ) -> Option<&VfsEntry<T>> {
        self.lookup(path, name.as_ref()).map(|(_, n)| n)
    }

    pub fn new_item(
        &mut self,
        dir: impl Borrow<VfsNode>,
        name: impl Into<SmartString<Compact>> + AsRef<str>,
        item: T,
    ) -> VfsResult<VfsNode> {
        self.new_node(dir, name, VfsEntry::Item { value: item })
    }

    pub fn new_dir(
        &mut self,
        path: impl Borrow<VfsNode>,
        name: impl Into<SmartString<Compact>> + AsRef<str>,
    ) -> VfsResult<VfsNode> {
        self.mkdir(path, name)
    }

    pub fn mkdir(
        &mut self,
        path: impl Borrow<VfsNode>,
        name: impl Into<SmartString<Compact>> + AsRef<str>,
    ) -> VfsResult<VfsNode> {
        self.new_node(path, name, VfsEntry::Dir)
    }

    /// Not very efficient due to lifetimes
    pub fn mkdir_p<N>(&mut self, mut path: impl Iterator<Item = N>) -> VfsResult<VfsNode>
    where
        N: Into<SmartString<Compact>> + AsRef<str>,
    {
        let root = self.root().clone();
        path.fold_while(Ok(root), |prev, next| match prev {
            Ok(prev) => match self.new_node(&prev, next, VfsEntry::Dir) {
                Ok(next) => FoldWhile::Continue(Ok(next)),
                e => FoldWhile::Done(e),
            },
            e => FoldWhile::Done(e),
        })
        .into_inner()
    }

    pub fn read(&self, path: &VfsNode) -> Option<&VfsEntry<T>> {
        self.inner.node_weight(path.index)
    }

    pub fn write(&mut self, path: &VfsNode) -> Option<&mut VfsEntry<T>> {
        self.inner.node_weight_mut(path.index)
    }

    pub fn rm(&mut self, path: &VfsNode) -> Option<VfsEntry<T>> {
        self.inner.remove_node(path.index)
    }

    pub fn iter(&self, path: &VfsNode) -> impl Iterator<Item = &VfsNode> {
        self.ls(path)
    }

    fn add_child(
        &mut self,
        parent: &VfsNode,
        child_name: SmartString<Compact>,
        node: VfsEntry<T>,
    ) -> &VfsNode {
        let child_path = parent.join(&child_name);
        let child_index = self.inner.add_node(node);

        let path = VfsNode {
            name: child_name.clone(),
            cached: SmartString::from(child_path.to_string()),
            inner: child_path,
            index: child_index,
        };

        let child_weight =
            self.inner
                .add_edge(parent.index, child_index, Relationship::Child(path));

        self.inner.add_edge(
            child_index,
            parent.index,
            Relationship::Parent(parent.clone()),
        );

        self.inner
            .edge_weight(child_weight)
            .expect("Edge was just added")
    }

    fn new_node(
        &mut self,
        path: impl Borrow<VfsNode>,
        name: impl Into<SmartString<Compact>> + AsRef<str>,
        node: VfsEntry<T>,
    ) -> VfsResult<VfsNode> {
        let path = path.borrow();
        let name = name.as_ref();

        if let Some((child_path, child_node)) = self.lookup(path, name) {
            if child_node.is_dir() && node.is_dir() {
                return Ok(child_path.clone());
            } else {
                return Err(VfsError::ItemAlreadyExists(child_path.clone()));
            }
        }

        Ok(self.add_child(path, name.into(), node).clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Relationship {
    /// This edge points to the node's parent
    Parent(VfsNode),
    /// This edge points to one of a node's children
    Child(VfsNode),
}

impl Deref for Relationship {
    type Target = VfsNode;

    fn deref(&self) -> &Self::Target {
        match self {
            Relationship::Parent(vfs_path) => vfs_path,
            Relationship::Child(vfs_path) => vfs_path,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Vfs;

    #[test]
    fn can_add_item_to_vfs() {
        let mut vfs = Vfs::new();
        let root = vfs.root().clone();
        let child = vfs
            .new_item(root, "child", 1)
            .expect("root dir should be empty");
        let entry = vfs.read(&child).expect("child was just added");
        let value = entry.value().expect("entry should be an item");
        assert_eq!(*value, 1);
    }

    #[test]
    fn can_add_dir_to_vfs() {
        let mut vfs = Vfs::<()>::new();
        let root = vfs.root().clone();
        let child = vfs.mkdir(root, "dir").expect("root should be empty");
        let entry = vfs.read(&child).expect("child was just made");
        assert!(entry.is_dir(), "entry should be a directory");
    }

    #[test]
    fn can_add_item_to_dir() {
        let mut vfs = Vfs::new();
        let root = vfs.root().clone();
        let child_dir = vfs.mkdir(root, "dir").expect("root should be empty");
        let grandchild_item = vfs
            .new_item(child_dir, "item", 1)
            .expect("new dir should be empty");
        let entry = vfs.read(&grandchild_item).expect("child was just added");
        let value = *entry.value().expect("entry should be an item");

        let root = vfs.root();
        let child_dir = vfs
            .lookup_path(root, "dir")
            .expect("dir should of been added");
        let grandchild_item = vfs
            .lookup_node(child_dir, "item")
            .expect("grandchild item should of been added");
        assert_eq!(grandchild_item.value().cloned(), Some(value));
    }
}
