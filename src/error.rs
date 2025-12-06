use crate::node::VfsNode;
use thiserror::Error;

pub type VfsResult<T = ()> = Result<T, VfsError>;

#[derive(Error, Debug)]
pub enum VfsError {
    #[error("The item at this path already exists")]
    ItemAlreadyExists(VfsNode),
    #[error("Only directories can have children")]
    InvalidParent(VfsNode),
}
