pub mod entry;
pub mod error;
pub mod node;

pub use crate::{
    entry::VfsEntry,
    error::{VfsError, VfsResult},
    node::VfsNode,
};

use bimap::BiHashMap;
use derive_enum_accessors::EnumFieldAccessors;
use itertools::{FoldWhile, Itertools};
use petgraph::{Graph, visit::EdgeRef};
use prehash::{DefaultPrehasher, Passthru, Prehashed, Prehasher};
use smartstring::{Compact, SmartString};
use std::{
    fmt::Debug,
    hash::{BuildHasherDefault, Hash},
};

type Ident = Prehashed<SmartString<Compact>>;
type IdentKey = Prehashed<usize>;
type IdentMap =
    BiHashMap<IdentKey, Ident, BuildHasherDefault<Passthru>, BuildHasherDefault<Passthru>>;

pub struct Vfs<T> {
    inner: Graph<VfsEntry<T>, Relationship>,
    root: VfsNode,

    idents: IdentMap,
    hasher: DefaultPrehasher,
}

impl<T> Default for Vfs<T> {
    fn default() -> Self {
        let mut graph = Graph::new();
        let mut idents = IdentMap::default();

        let hasher = DefaultPrehasher::new();

        let root_key = Self::get_or_make_ident(&mut idents, &hasher, "/");
        let root_index = graph.add_node(VfsEntry::Dir);

        Self {
            inner: graph,
            root: VfsNode {
                ident: root_key,
                index: root_index,
            },
            idents,
            hasher,
        }
    }
}

impl<T> Vfs<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn root(&self) -> VfsNode {
        self.root
    }

    pub fn find_absolute(&self, path: impl AsRef<str>) -> Option<VfsNode> {
        self.inner
            .edge_references()
            .find_map(|edge| match edge.weight() {
                Relationship::Parent { .. } => None,
                Relationship::Child { node } => node.absolute(self).and_then(|ap| {
                    if ap == path.as_ref() {
                        Some(*node)
                    } else {
                        None
                    }
                }),
            })
    }

    pub fn ls(&self, node: VfsNode) -> impl Iterator<Item = VfsNode> {
        self.inner
            .edges(node.index)
            .map(|e| e.weight())
            .cloned()
            .filter_map(|relationship| {
                if let Relationship::Child { node } = relationship {
                    Some(node)
                } else {
                    None
                }
            })
    }

    pub fn lookup(&self, node: VfsNode, name: impl AsRef<str>) -> Option<(VfsNode, &VfsEntry<T>)> {
        let name = self.hasher.prehash(SmartString::from(name.as_ref()));
        self.inner.edges(node.index).find_map(|e| {
            self.ident_of(*e.weight().node()).and_then(|ident| {
                if Prehashed::fast_eq(ident, &name) {
                    Some(*e.weight().node()).zip(self.inner.node_weight(e.target()))
                } else {
                    None
                }
            })
        })
    }

    pub fn search(&self, name: impl AsRef<str>) -> Vec<VfsNode> {
        let search_str = name.as_ref().to_lowercase();
        self.inner
            .edge_weights()
            .filter_map(|w| {
                let node = *w.node();
                self.ident_of(node).and_then(|ident| {
                    if ident.to_lowercase().contains(&search_str) {
                        Some(node)
                    } else {
                        None
                    }
                })
            })
            .unique()
            .collect()
    }

    pub fn lookup_path(&self, node: VfsNode, name: impl AsRef<str>) -> Option<VfsNode> {
        self.lookup(node, name.as_ref()).map(|(p, _)| p)
    }

    pub fn lookup_node(&self, node: VfsNode, name: impl AsRef<str>) -> Option<&VfsEntry<T>> {
        self.lookup(node, name.as_ref()).map(|(_, n)| n)
    }

    pub fn new_item(
        &mut self,
        dir: VfsNode,
        name: impl Into<SmartString<Compact>> + AsRef<str>,
        item: T,
    ) -> VfsResult<VfsNode> {
        self.new_node(dir, name, VfsEntry::Item { value: item })
    }

    pub fn new_dir(
        &mut self,
        node: VfsNode,
        name: impl Into<SmartString<Compact>> + AsRef<str>,
    ) -> VfsResult<VfsNode> {
        self.mkdir(node, name)
    }

    pub fn mkdir(
        &mut self,
        node: VfsNode,
        name: impl Into<SmartString<Compact>> + AsRef<str>,
    ) -> VfsResult<VfsNode> {
        self.new_node(node, name, VfsEntry::Dir)
    }

    /// Not very efficient due to lifetimes
    pub fn mkdir_p<N>(&mut self, mut path: impl Iterator<Item = N>) -> VfsResult<VfsNode>
    where
        N: Into<SmartString<Compact>> + AsRef<str>,
    {
        let root = self.root();
        path.fold_while(Ok(root), |prev, next| match prev {
            Ok(prev) => match self.new_node(prev, next, VfsEntry::Dir) {
                Ok(next) => FoldWhile::Continue(Ok(next)),
                e => FoldWhile::Done(e),
            },
            e => FoldWhile::Done(e),
        })
        .into_inner()
    }

    pub fn rm(&mut self, path: &VfsNode) -> Option<VfsEntry<T>> {
        self.inner.remove_node(path.index)
    }

    pub fn read(&self, node: VfsNode) -> Option<&VfsEntry<T>> {
        self.inner.node_weight(node.index)
    }

    pub fn write(&mut self, node: VfsNode) -> Option<&mut VfsEntry<T>> {
        self.inner.node_weight_mut(node.index)
    }

    fn add_child(
        &mut self,
        parent_node: VfsNode,
        child_name: SmartString<Compact>,
        node: VfsEntry<T>,
    ) -> VfsResult<VfsNode> {
        if let Some(parent_entry) = self.read(parent_node)
            && !parent_entry.is_dir()
        {
            return Err(VfsError::InvalidParent(parent_node));
        }

        let child_index = self.inner.add_node(node);

        let ident = Self::get_or_make_ident(&mut self.idents, &self.hasher, child_name);

        let child_node = VfsNode {
            ident,
            index: child_index,
        };

        self.inner.add_edge(
            parent_node.index,
            child_index,
            Relationship::Child { node: child_node },
        );

        self.inner.add_edge(
            child_index,
            parent_node.index,
            Relationship::Parent { node: parent_node },
        );

        Ok(child_node)
    }

    fn new_node(
        &mut self,
        node: VfsNode,
        name: impl Into<SmartString<Compact>> + AsRef<str>,
        entry: VfsEntry<T>,
    ) -> VfsResult<VfsNode> {
        let name = name.as_ref();

        if let Some((child_path, child_node)) = self.lookup(node, name) {
            if child_node.is_dir() && entry.is_dir() {
                return Ok(child_path);
            } else {
                return Err(VfsError::ItemAlreadyExists(child_path));
            }
        }

        self.add_child(node, name.into(), entry)
    }

    fn ident_of(&self, node: VfsNode) -> Option<&Ident> {
        self.idents.get_by_left(&node.ident)
    }

    fn get_or_make_ident(
        ident_map: &mut IdentMap,
        hasher: &DefaultPrehasher,
        name: impl AsRef<str>,
    ) -> IdentKey {
        let ident = hasher.prehash(SmartString::from(name.as_ref()));
        ident_map.get_by_right(&ident).cloned().unwrap_or_else(|| {
            let key = hasher.prehash(ident_map.len());
            ident_map.insert(key, ident);
            key
        })
    }
}

#[derive(EnumFieldAccessors, Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Relationship {
    /// This edge points to the node's parent
    /// child (source) -> parent (target)
    Parent { node: VfsNode },
    /// This edge points to one of a node's children
    /// parent (source) -> children (target)
    Child { node: VfsNode },
}

#[cfg(test)]
mod tests {
    use crate::Vfs;

    #[test]
    fn can_add_item_to_vfs() {
        let mut vfs = Vfs::new();
        let root = vfs.root();
        let child = vfs
            .new_item(root, "child", 1)
            .expect("root dir should be empty");
        let entry = vfs.read(child).expect("child was just added");
        let value = entry.value().expect("entry should be an item");
        assert_eq!(*value, 1);
    }

    #[test]
    fn can_add_dir_to_vfs() {
        let mut vfs = Vfs::<()>::new();
        let root = vfs.root();
        let child = vfs.mkdir(root, "dir").expect("root should be empty");
        let entry = vfs.read(child).expect("child was just made");
        assert!(entry.is_dir(), "entry should be a directory");
    }

    #[test]
    fn can_add_item_to_dir() {
        let mut vfs = Vfs::new();
        let root = vfs.root();
        let child_dir = vfs.mkdir(root, "dir").expect("root should be empty");
        let grandchild_item = vfs
            .new_item(child_dir, "item", 1)
            .expect("new dir should be empty");
        let entry = vfs.read(grandchild_item).expect("child was just added");
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

    #[test]
    fn can_resolve_full_paths() {
        let mut vfs = Vfs::new();

        let root = vfs.root();
        let child_dir = vfs.mkdir(root, "dir").expect("root should be empty");
        let grandchild_item = vfs
            .new_item(child_dir, "item", 1)
            .expect("new dir should be empty");

        let abs_path = grandchild_item
            .absolute(&vfs)
            .expect("absolute path should be resolvable");

        assert_eq!(abs_path, "/dir/item");
    }
}
