use derive_enum_accessors::EnumFieldAccessors;
use std::fmt::Debug;

#[derive(EnumFieldAccessors)]
pub enum VfsEntry<T> {
    Dir,
    Item { value: T },
}

impl<T> VfsEntry<T> {
    pub fn is_item(&self) -> bool {
        matches!(self, Self::Item { .. })
    }

    pub fn is_dir(&self) -> bool {
        matches!(self, Self::Dir)
    }
}

impl<T> Clone for VfsEntry<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::Dir => Self::Dir,
            Self::Item { value } => Self::Item {
                value: value.clone(),
            },
        }
    }
}

impl<T> Debug for VfsEntry<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VfsEntry::Dir => f.debug_tuple(std::any::type_name::<Self>()).finish(),
            VfsEntry::Item { value } => f
                .debug_struct(std::any::type_name::<Self>())
                .field("value", value)
                .finish(),
        }
    }
}
