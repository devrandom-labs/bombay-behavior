//! Mechanical derivation of the shared named request-product contracts.

macro_rules! request_product {
    (
        $(#[$attribute:meta])*
        $visibility:vis struct $name:ident<$($parameter:ident),+ $(,)?> {
            $($field_visibility:vis $field:ident: $field_type:ident),+ $(,)?
        }
    ) => {
        $(#[$attribute])*
        $visibility struct $name<$($parameter),+> {
            $($field_visibility $field: $field_type),+
        }

        impl<$($parameter),+> behavior::SendEffects for $name<$($parameter),+>
        where
            $($parameter: behavior::SendEffects),+
        {
            fn empty() -> Self {
                Self {
                    $($field: <$field_type as behavior::SendEffects>::empty()),+
                }
            }

            fn append(&mut self, other: Self) {
                $(
                    <$field_type as behavior::SendEffects>::append(
                        &mut self.$field,
                        other.$field,
                    );
                )+
            }
        }

        impl<Event, $($parameter),+> behavior::SendsFor<Event> for $name<$($parameter),+>
        where
            $($parameter: behavior::SendEffects + behavior::SendsFor<Event>),+
        {
        }

        impl<$($parameter),+> behavior::SendSettlements for $name<$($parameter),+>
        where
            $($parameter: behavior::SendSettlements),+
        {
            type Settlements = $name<
                $(<$field_type as behavior::SendSettlements>::Settlements),+
            >;

            fn unattempted(self) -> Self::Settlements {
                $name {
                    $(
                        $field: <$field_type as behavior::SendSettlements>::unattempted(
                            self.$field,
                        )
                    ),+
                }
            }
        }

        impl<$($parameter),+> behavior::ClassifySettlement for $name<$($parameter),+>
        where
            $($parameter: behavior::ClassifySettlement),+
        {
            fn settlement_status(&self) -> behavior::SettlementStatus {
                let status = behavior::SettlementStatus::Accepted;
                $(
                    let status = status.combine(
                        behavior::ClassifySettlement::settlement_status(&self.$field),
                    );
                )+
                status
            }
        }

        impl<Host, RootEvent, $($parameter),+>
            behavior::SourceSettlementCustody<Host, RootEvent>
            for $name<$($parameter),+>
        where
            Host: Send,
            $($parameter: behavior::SourceSettlementCustody<Host, RootEvent> + Send),+
        {
            fn offer_next_to_source(
                self,
                host: &mut Host,
            ) -> impl core::future::Future<
                Output = behavior::SourceCustody<Self>,
            > + Send {
                async move {
                    enum TerminalCustody {
                        Unrequired,
                        Required,
                    }
                    let mut terminal_custody = TerminalCustody::Unrequired;
                    request_product! {
                        @custody
                        $name,
                        self,
                        host,
                        terminal_custody,
                        [],
                        [$(($field, $field_type)),+]
                    }
                }
            }
        }

        impl<Interpreter, RootEvent, Path, $($parameter),+>
            behavior::InterpretSends<Interpreter, RootEvent, Path>
            for $name<$($parameter),+>
        where
            Interpreter: Send,
            $($parameter: behavior::InterpretSends<Interpreter, RootEvent, Path>),+
        {
            fn interpret(
                self,
                interpreter: &mut Interpreter,
            ) -> impl core::future::Future<
                Output = behavior::Interpretation<Self::Settlements>,
            > + Send {
                async move {
                    request_product! {
                        @interpret
                        $name,
                        self,
                        interpreter,
                        [],
                        [$(($field, $field_type)),+]
                    }
                }
            }
        }
    };
    (
        @custody
        $name:ident,
        $owner:ident,
        $host:ident,
        $terminal_custody:ident,
        [$($settled:ident),*],
        [($field:ident, $field_type:ident) $(, ($later:ident, $later_type:ident))*]
    ) => {
        match <$field_type as behavior::SourceSettlementCustody<_, _>>::offer_next_to_source(
            $owner.$field,
            $host,
        ).await {
            behavior::SourceCustody::Exhausted($field) => {
                request_product! {
                    @custody
                    $name,
                    $owner,
                    $host,
                    $terminal_custody,
                    [$($settled,)* $field],
                    [$(($later, $later_type)),*]
                }
            }
            behavior::SourceCustody::Retained($field) => {
                $terminal_custody = TerminalCustody::Required;
                request_product! {
                    @custody
                    $name,
                    $owner,
                    $host,
                    $terminal_custody,
                    [$($settled,)* $field],
                    [$(($later, $later_type)),*]
                }
            }
            behavior::SourceCustody::Admitted($field) => {
                behavior::SourceCustody::Admitted($name {
                    $($settled: $settled,)*
                    $field,
                    $($later: $owner.$later),*
                })
            }
            behavior::SourceCustody::Closed($field) => {
                behavior::SourceCustody::Closed($name {
                    $($settled: $settled,)*
                    $field,
                    $($later: $owner.$later),*
                })
            }
        }
    };
    (
        @custody
        $name:ident,
        $owner:ident,
        $host:ident,
        $terminal_custody:ident,
        [$($settled:ident),+],
        []
    ) => {
        {
            let settlements = $name {
                $($settled: $settled),+
            };
            match $terminal_custody {
                TerminalCustody::Unrequired => {
                    behavior::SourceCustody::Exhausted(settlements)
                }
                TerminalCustody::Required => {
                    behavior::SourceCustody::Retained(settlements)
                }
            }
        }
    };
    (
        @interpret
        $name:ident,
        $owner:ident,
        $interpreter:ident,
        [$($settled:ident),*],
        [($field:ident, $field_type:ident) $(, ($later:ident, $later_type:ident))*]
    ) => {
        match <$field_type as behavior::InterpretSends<_, _, _>>::interpret(
            $owner.$field,
            $interpreter,
        ).await {
            behavior::Interpretation::Complete($field) => {
                request_product! {
                    @interpret
                    $name,
                    $owner,
                    $interpreter,
                    [$($settled,)* $field],
                    [$(($later, $later_type)),*]
                }
            }
            behavior::Interpretation::Corrupt($field) => {
                behavior::Interpretation::Corrupt($name {
                    $($settled: $settled,)*
                    $field,
                    $(
                        $later: <$later_type as behavior::SendSettlements>::unattempted(
                            $owner.$later,
                        ),
                    )*
                })
            }
        }
    };
    (
        @interpret
        $name:ident,
        $owner:ident,
        $interpreter:ident,
        [$($settled:ident),+],
        []
    ) => {
        behavior::Interpretation::Complete($name {
            $($settled: $settled),+
        })
    };
}

pub(super) use request_product;
