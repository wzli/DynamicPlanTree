pub use behaviour::Behaviour;
pub use enum_dispatch::enum_dispatch;
pub use plan::*;
pub use predicate::Predicate;
pub use serde::{Deserialize, Serialize};

pub mod behaviour;
pub mod plan;
pub mod predicate;

pub trait FromAny: Sized {
    fn from_any(x: impl std::any::Any) -> Option<Self>;
}

#[macro_export]
macro_rules! from_any {
    ($($src:ty),* $(,)?) => {
        fn from_any(x: impl std::any::Any) -> Option<Self> {
            let mut x = Some(x);
            let _x = &mut x as &mut dyn std::any::Any;
            $(
                if let Some(x) = _x.downcast_mut::<Option<$src>>() {
                    std::mem::take(x).map(|x| x.into())
                } else
             )*
            { None }
        }
    }
}
