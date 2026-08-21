//! Cross-adapter proofs for the shared fixed-fleet ownership fold.

use std::time::{Duration, Instant};

use behavior::{
    Acted, Actions, Activate as _, ChildTopology, Crash, Create, CreationResolved, MailAddr, Never,
    RestartConfiguration, RestartPolicy, Strategy, Supervise, SupervisionEvent, Supervisor, User,
    WorkerStopped,
};

struct Child;

#[behavior::behavior(addr = MailAddr, message = (), sends = Vec<Never>, births = behavior::NoBirths, error = Never)]
impl Child {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: (),
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::NoBirths, Never> {
        Ok(Actions::cont())
    }
}

/// A real application composition: user input stages an additional child.
struct Application;

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = behavior::Births<Child>, error = Never)]
impl Application {
    fn receive(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::Births<Child>, Never> {
        Ok(Actions::create(vec![Create::birth(nonce, Child)]))
    }
}

fn child(_: usize) -> Option<Child> {
    Some(Child)
}

fn restart() -> RestartConfiguration {
    RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        4,
        Duration::from_secs(10),
    )
}

#[test]
fn standalone_and_composed_adapters_preserve_the_same_fixed_fleet_trace() {
    let topology = || ChildTopology::new([3, 5], child);
    let standalone = Supervisor::<MailAddr, Child>::new(topology(), restart()).unwrap();
    let composed = Supervise::new(Application, topology(), restart()).unwrap();
    let standalone = standalone.initialize().unwrap();
    let composed = composed.initialize().unwrap();

    assert_eq!(
        standalone
            .actions
            .creates
            .iter()
            .map(|create| create.nonce)
            .collect::<Vec<_>>(),
        composed
            .actions
            .creates
            .iter()
            .map(|create| create.nonce)
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        standalone.actions.sends.child_observations.as_slice(),
        composed.actions.sends.owned.child_observations.as_slice(),
    );

    let mut standalone = standalone.behavior;
    let mut composed = composed.behavior;
    for nonce in [3, 5] {
        let fact = CreationResolved::birth(nonce, MailAddr(100 + nonce));
        standalone.on(fact).unwrap();
        composed.on(fact).unwrap();
    }
    let stopped = WorkerStopped::new(3, 103, Err(Crash::Failed), Instant::now());
    let standalone_actions = standalone.on(stopped.clone()).unwrap();
    let composed_actions = composed.on(stopped).unwrap();

    let destinations = |commands: &[foundation::ChildDelivery<
        behavior::Proxy<Child>,
        foundation::ChildHead,
    >]| {
        commands
            .iter()
            .map(|delivery| delivery.nonce)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        destinations(&standalone_actions.sends.replacement_commands),
        destinations(&composed_actions.sends.owned.replacement_commands),
    );
    assert_eq!(standalone.child_count(), composed.child_count());
    assert_eq!(
        standalone.restarts_in_window(),
        composed.restarts_in_window()
    );
}

#[test]
fn composed_application_birth_is_adopted_after_the_configured_initialization_batch() {
    let initialized = Supervise::new(Application, ChildTopology::new([3], child), restart())
        .unwrap()
        .initialize()
        .unwrap();
    let mut active = initialized.behavior;
    let actions = active
        .transition(SupervisionEvent::Behavior(User::new(MailAddr(1), 11)))
        .unwrap();
    assert_eq!(actions.creates.len(), 1);
    assert_eq!(actions.creates[0].nonce, 11);
    assert_eq!(actions.sends.owned.child_observations[0].nonce, 11);
    assert_eq!(actions.sends.owned.creation_observations[0].nonce, 11);
}
