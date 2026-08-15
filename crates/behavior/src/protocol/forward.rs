//! Crate-private syntax for the common composition law: a wrapper delegates
//! an event lane to its inner protocol without changing the payload.

macro_rules! forward_event_lane {
    ($wrapper:ident, $event:ty) => {
        $crate::protocol::forward::forward_event_lane!($wrapper, $event, Behavior);
    };
    ($wrapper:ident, $event:ty, $wrapped:ident) => {
        impl<E> $crate::RouteInput<$event> for $wrapper<E>
        where
            E: $crate::UserEvent + $crate::RouteInput<$event>,
        {
            fn route(event: $event) -> Result<Self, $event> {
                E::route(event).map(Self::$wrapped)
            }
        }

        impl<E> $crate::EventInput<$event> for $wrapper<E>
        where
            E: $crate::UserEvent + $crate::EventInput<$event>,
        {
            fn inject(event: $event) -> Self {
                Self::$wrapped(E::inject(event))
            }
        }
    };
}

pub(crate) use forward_event_lane;
