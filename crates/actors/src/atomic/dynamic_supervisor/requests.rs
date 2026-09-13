//! DynamicSupervisor request order.

use crate::atomic::requests::request_product;

request_product! {
    /// Named request lanes in their declared interpretation order.
    #[doc(hidden)]
    pub struct DynamicSupervisorRequests<
        ProxyObservations,
        ProxyOperations,
        ShutdownSchedules,
        StartReplies,
        ReplaceReplies,
        StopReplies,
        QueryReplies,
        CancelReplies,
        Lifecycle,
        Diagnostics,
    > {
        pub proxy_observations: ProxyObservations,
        pub proxy_operations: ProxyOperations,
        pub shutdown_schedules: ShutdownSchedules,
        pub start_replies: StartReplies,
        pub replace_replies: ReplaceReplies,
        pub stop_replies: StopReplies,
        pub query_replies: QueryReplies,
        pub cancel_replies: CancelReplies,
        pub lifecycle: Lifecycle,
        pub diagnostics: Diagnostics,
    }
}
