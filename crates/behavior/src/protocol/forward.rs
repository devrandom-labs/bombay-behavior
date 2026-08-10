//! Crate-private syntax for the common composition law: a wrapper delegates
//! an event lane to its inner protocol without changing the payload.

macro_rules! forward_event_lane {
    ($wrapper:ident, $trait:ident, $method:ident, $event:ty) => {
        impl<E: $crate::$trait> $crate::$trait for $wrapper<E> {
            fn $method(event: $event) -> Option<Self> {
                E::$method(event).map(Self::Inner)
            }
        }

        impl<E> $crate::EventInput<$event> for $wrapper<E>
        where
            E: $crate::$trait + $crate::EventInput<$event>,
        {
            fn inject(event: $event) -> Self {
                Self::Inner(E::inject(event))
            }
        }
    };
}

pub(crate) use forward_event_lane;
