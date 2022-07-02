pub use enum_cast_derive::EnumCast;

/// Macro to get reference to inner struct of specified variant.
#[macro_export]
macro_rules! enum_cast {
    ($enum: expr, $var: path $(,)?) => {{
        match &$enum {
            $var(a) => Some(a),
            _ => None,
        }
    }};
}

/// Trait to convert to reference of inner struct for each variant.
pub trait EnumRef<T> {
    fn enum_ref(&self) -> Option<&T>;
    fn enum_mut(&mut self) -> Option<&mut T>;
}

/// Trait to convert to and from any object, useful when variants are not statically known.
pub trait EnumCast {
    fn cast<T: 'static>(&self) -> Option<&T>;
    fn cast_mut<T: 'static>(&mut self) -> Option<&mut T>;
    fn from_any<T: 'static>(t: T) -> Option<Self>
    where
        Self: Sized;
}

/// Trait for reverse of EnumCast::from_any to allow type inference with blanket implementation.
pub trait IntoEnum {
    fn into_enum<T: EnumCast>(self) -> Option<T>
    where
        Self: 'static + Sized,
    {
        T::from_any(self)
    }
}

impl<T> IntoEnum for T {}
