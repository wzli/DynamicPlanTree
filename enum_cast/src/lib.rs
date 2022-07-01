pub use enum_cast_derive::EnumCast;

pub trait EnumRef<T> {
    fn enum_ref(&self) -> Option<&T>;
    fn enum_mut(&mut self) -> Option<&mut T>;
}

pub trait EnumCast {
    fn cast<T: 'static>(&self) -> Option<&T>;
    fn cast_mut<T: 'static>(&mut self) -> Option<&mut T>;
    fn from_any<T: 'static>(t: T) -> Option<Self>
    where
        Self: Sized;
}

pub trait IntoEnum {
    fn into_enum<T: EnumCast>(self) -> Option<T>
    where
        Self: 'static + Sized,
    {
        T::from_any(self)
    }
}

impl<T> IntoEnum for T {}
