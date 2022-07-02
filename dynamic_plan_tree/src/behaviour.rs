pub use crate::*;

/// Macro to redefine `Behaviour` trait in external crates for remote enum_dispatch definition.
#[macro_export]
macro_rules! behaviour_trait {
    () => {
        /// An object that implements run-time behaviour logic of an active plan.
        #[enum_dispatch]
        pub trait Behaviour<C: Config>: Sized + 'static {
            /// State of the plan's objective. May be queried while inactive.
            ///
            /// **In Progress** := `None` **Success** := `Some(true)` **Failure** := `Some(false)`
            fn status(&self, plan: &Plan<C>) -> Option<bool>;
            /// Value of the plan under current circumstances. May be queried while inactive.
            fn utility(&self, _plan: &Plan<C>) -> f64 {
                0.
            }
            /// Triggers once upon becoming active.
            fn on_entry(&mut self, _plan: &mut Plan<C>) {}
            /// Triggers once upon becoming inactive.
            fn on_exit(&mut self, _plan: &mut Plan<C>) {}
            /// Triggers before each run. Executes before subplans if scheduled on the same tick.
            fn on_prepare(&mut self, _plan: &mut Plan<C>) {}
            /// Triggers repeatedly while active. Executes after subplans if scheduled on the same tick.
            fn on_run(&mut self, _plan: &mut Plan<C>) {}
        }
    };
}
behaviour_trait!();

/// Default set of built-in behaviours to serve as example template.
#[enum_dispatch(Behaviour<C>)]
#[derive(EnumCast)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Behaviours<C: Config> {
    AllSuccessStatus,
    AnySuccessStatus,
    EvaluateStatus(EvaluateStatus<C>),
    ModifyStatus(ModifyStatus<C>),

    MultiBehaviour(MultiBehaviour<C>),
    RepeatBehaviour(RepeatBehaviour<C>),
    SequenceBehaviour,
    FallbackBehaviour,
    MaxUtilBehaviour,
}

/// Returns `false` if `f.evaluate()`, `true` if `t.evaluate()`, otherwise `None`.
pub fn evaluate_status<C: Config, T: Predicate, F: Predicate>(
    plan: &Plan<C>,
    t: &T,
    f: &F,
) -> Option<bool> {
    if f.evaluate(plan, &[]) {
        Some(false)
    } else if t.evaluate(plan, &[]) {
        Some(true)
    } else {
        None
    }
}

/// Behaviour with status that invokes `evaluate_status(&self.0, &self.1)`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EvaluateStatus<C: Config>(pub C::Predicate, pub C::Predicate);
impl<C: Config> Behaviour<C> for EvaluateStatus<C> {
    fn status(&self, plan: &Plan<C>) -> Option<bool> {
        evaluate_status(plan, &self.0, &self.1)
    }
}

/// Behaviour with status `true` if `AllSuccess`, `false` if `AnyFailure`, otherwise `None`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AllSuccessStatus;
impl<C: Config> Behaviour<C> for AllSuccessStatus {
    fn status(&self, plan: &Plan<C>) -> Option<bool> {
        evaluate_status(plan, &predicate::AllSuccess, &predicate::AnyFailure)
    }
}

/// Behaviour with status `true` if `AnySuccess`, `false` if `AllFailure`, otherwise `None`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AnySuccessStatus;
impl<C: Config> Behaviour<C> for AnySuccessStatus {
    fn status(&self, plan: &Plan<C>) -> Option<bool> {
        evaluate_status(plan, &predicate::AnySuccess, &predicate::AllFailure)
    }
}

/// Wraps inner behaviour. If inner status exists, invert when `self.1` is `None` otherwise use `self.1`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ModifyStatus<C: Config>(pub Box<C::Behaviour>, pub Option<bool>);
impl<C: Config> Behaviour<C> for ModifyStatus<C> {
    fn status(&self, plan: &Plan<C>) -> Option<bool> {
        self.0.status(plan).map(|x| self.1.unwrap_or(!x))
    }
    fn utility(&self, plan: &Plan<C>) -> f64 {
        self.0.utility(plan)
    }
    fn on_entry(&mut self, plan: &mut Plan<C>) {
        self.0.on_entry(plan);
    }
    fn on_exit(&mut self, plan: &mut Plan<C>) {
        self.0.on_exit(plan);
    }
    fn on_prepare(&mut self, plan: &mut Plan<C>) {
        self.0.on_prepare(plan);
    }
    fn on_run(&mut self, plan: &mut Plan<C>) {
        self.0.on_run(plan);
    }
}

/// Vector of behaviours sharing the same plan. Status takes aggregate AND. Utility takes aggregate sum.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MultiBehaviour<C: Config>(pub Vec<C::Behaviour>);
impl<C: Config> Behaviour<C> for MultiBehaviour<C> {
    fn status(&self, plan: &Plan<C>) -> Option<bool> {
        let mut status = Some(true);
        for behaviour in &self.0 {
            match behaviour.status(plan) {
                Some(true) => {}
                Some(false) => return Some(false),
                None => status = None,
            }
        }
        status
    }
    fn utility(&self, plan: &Plan<C>) -> f64 {
        self.0.iter().map(|behaviour| behaviour.utility(plan)).sum()
    }
    fn on_entry(&mut self, plan: &mut Plan<C>) {
        for behaviour in &mut self.0 {
            behaviour.on_entry(plan);
        }
    }
    fn on_exit(&mut self, plan: &mut Plan<C>) {
        for behaviour in &mut self.0 {
            behaviour.on_exit(plan);
        }
    }
    fn on_prepare(&mut self, plan: &mut Plan<C>) {
        for behaviour in &mut self.0 {
            behaviour.on_prepare(plan);
        }
    }
    fn on_run(&mut self, plan: &mut Plan<C>) {
        for behaviour in &mut self.0 {
            behaviour.on_run(plan);
        }
    }
}

/// Repeats inner behaviour for specified iterations until failure encountered while condition holds.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RepeatBehaviour<C: Config> {
    /// Behaviour that expects some status on completion to mark each iteration.
    pub behaviour: Box<C::Behaviour>,
    /// Stop running behaviour once condition no longer holds.
    pub condition: Option<C::Predicate>,
    /// Stop running behaviour after specified iterations.
    pub iterations: usize,
    /// Repeat until behaviour status returns `retry` (either success or failure).
    pub retry: bool,

    count_down: usize,
    status: Option<bool>,
}

impl<C: Config> RepeatBehaviour<C> {
    pub fn new(behaviour: C::Behaviour) -> Self {
        Self {
            behaviour: Box::new(behaviour),
            condition: None,
            iterations: usize::MAX,
            retry: false,
            count_down: 0,
            status: None,
        }
    }
}

impl<C: Config> Behaviour<C> for RepeatBehaviour<C> {
    fn status(&self, _plan: &Plan<C>) -> Option<bool> {
        self.status
    }
    fn utility(&self, plan: &Plan<C>) -> f64 {
        self.behaviour.utility(plan)
    }
    fn on_entry(&mut self, plan: &mut Plan<C>) {
        self.status = None;
        self.count_down = self.iterations;
        self.behaviour.on_entry(plan);
    }
    fn on_exit(&mut self, plan: &mut Plan<C>) {
        self.behaviour.on_exit(plan);
    }
    fn on_prepare(&mut self, plan: &mut Plan<C>) {
        // run only while status is indeterminant
        if self.status.is_some() {
            return;
        }
        // stop when countdown runs out or condition doesn't hold
        if self.count_down == 0
            || !self
                .condition
                .as_ref()
                .map(|x| x.evaluate(plan, &[]))
                .unwrap_or(true)
        {
            self.status = Some(!self.retry);
            return;
        }
        self.behaviour.on_prepare(plan);
    }
    fn on_run(&mut self, plan: &mut Plan<C>) {
        // run only while status is indeterminant
        if self.status.is_some() {
            return;
        }
        self.behaviour.on_run(plan);
        // tick countdown only when inner behaviour return some status
        if let Some(status) = self.behaviour.status(plan) {
            if status != self.retry {
                // if success, decrement countdown and reset behaviour
                self.count_down -= 1;
                self.behaviour.on_exit(plan);
                self.behaviour.on_entry(plan);
            } else {
                // if failure, store status and stop
                self.status = Some(self.retry);
            }
        }
    }
}

/// Behaviour that sequentially transitions through child plans until first failure.
///
/// # Transitions
/// Plan is expected to contain transitions that form a linear sequence of success predicates,
/// with only one child plan active at a time. Behaviour is undefined otherwise.
///
/// If the status of any previously visited child plan changes from success,
/// the sequence will transition back to that point.

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SequenceBehaviour(Vec<String>);
impl<C: Config> Behaviour<C> for SequenceBehaviour {
    /// - Success when all child plans succeed.
    /// - Failure when any child plan fails.
    /// - None while otherwise in-progress.
    fn status(&self, plan: &Plan<C>) -> Option<bool> {
        AllSuccessStatus.status(plan)
    }
    fn on_prepare(&mut self, plan: &mut Plan<C>) {
        check_visited_status_and_jump(plan, &mut self.0, false);
    }
}

/// Behaviour that sequentially transitions through child plans until first success.
///
/// # Transitions
/// Plan is expected to contain transitions that form a linear sequence of failure predicates,
/// with only one child plan active at a time. Behaviour is undefined otherwise.
///
/// If the status of any previously visited child plan changes from failure,
/// the sequence will transition back to that point.

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FallbackBehaviour(Vec<String>);
impl<C: Config> Behaviour<C> for FallbackBehaviour {
    /// - Success when any child plans succeeds.
    /// - Failure when all child plan fail.
    /// - None while otherwise in-progress.
    fn status(&self, plan: &Plan<C>) -> Option<bool> {
        AnySuccessStatus.status(plan)
    }
    fn on_prepare(&mut self, plan: &mut Plan<C>) {
        check_visited_status_and_jump(plan, &mut self.0, true);
    }
}

fn check_visited_status_and_jump<C: Config>(
    plan: &mut Plan<C>,
    visited: &mut Vec<String>,
    jump_val: bool,
) {
    // find first inactive visited plans that has status none
    let pos = visited.iter().position(|x| match plan.get(x) {
        Some(x) => !x.active() && x.status().map(|x| x == jump_val).unwrap_or(true),
        None => false,
    });
    // jump back to that plan
    if let Some(pos) = pos {
        plan.exit(true);
        plan.enter_plan(&visited[pos]);
        visited.truncate(pos);
    }
    // find currently active plan
    let active = match plan.plans.iter().find(|x| x.active()) {
        Some(x) => x.name(),
        None => return,
    };
    // add active plan to visited if not already
    if let Some(last) = visited.last() {
        if last == active {
            return;
        }
    }
    visited.push(active.clone());
}

/// Behaviour that monitors and transitions to the child plan with highest utility.
///
/// Plan is expected to contain no transitions, with only one child active at a time. Behaviour is undefined otherwise.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MaxUtilBehaviour;
impl<C: Config> Behaviour<C> for MaxUtilBehaviour {
    /// Returns status of currently active child plan.
    fn status(&self, plan: &Plan<C>) -> Option<bool> {
        plan.plans.iter().find(|p| p.active())?.status()
    }
    /// Returns max utility of all child plans.
    fn utility(&self, plan: &Plan<C>) -> f64 {
        match max_utility(&plan.plans) {
            Some((_, util)) => util,
            None => 0.,
        }
    }
    fn on_prepare(&mut self, plan: &mut Plan<C>) {
        // get highest utility plan
        let best = match max_utility(&plan.plans) {
            Some((plan, _)) => plan.name().clone(),
            None => return,
        };
        // get active plan
        if let Some(active_plan) = plan.plans.iter().find(|plan| plan.active()) {
            // current plan is already best
            if *active_plan.name() == best {
                return;
            }
            // exit active plan
            let active = active_plan.name().clone();
            plan.exit_plan(&active);
        }
        // enter new plan
        plan.enter_plan(&best);
    }
}

/// Find and return the plan with highest utility.
pub fn max_utility<C: Config>(plans: &[Plan<C>]) -> Option<(&Plan<C>, f64)> {
    if plans.is_empty() {
        None
    } else {
        let (pos, utility) = plans
            .iter()
            .map(|plan| plan.utility())
            .enumerate()
            .fold((0, f64::NAN), |max, x| if max.1 > x.1 { max } else { x });
        Some((&plans[pos], utility))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    struct DefaultConfig;
    impl Config for DefaultConfig {
        type Predicate = predicate::Predicates;
        type Behaviour = behaviour::Behaviours<Self>;
    }
    type DC = DefaultConfig;

    #[test]
    fn evaluate_status() {
        let make_plan = |t: bool, f: bool| {
            Plan::<DC>::new(
                EvaluateStatus(
                    if t {
                        predicate::True.into()
                    } else {
                        predicate::False.into()
                    },
                    if f {
                        predicate::True.into()
                    } else {
                        predicate::False.into()
                    },
                )
                .into(),
                "root",
                1,
                true,
            )
        };

        let plan = make_plan(false, false);
        assert_eq!(plan.status(), None);

        let plan = make_plan(false, true);
        assert_eq!(plan.status(), Some(false));

        let plan = make_plan(true, false);
        assert_eq!(plan.status(), Some(true));

        let plan = make_plan(true, true);
        assert_eq!(plan.status(), Some(false));
    }

    #[test]
    fn repeat_behaviour() {
        //use tracing::info;
        //let _ = tracing_subscriber::fmt::try_init();
        let mut repeat = RepeatBehaviour::new(AllSuccessStatus.into());
        repeat.iterations = 5;
        let mut plan = Plan::<DC>::new(repeat.into(), "root", 1, true);
        // test iteration limit
        for _ in 0..5 {
            plan.run();
            assert_eq!(plan.status(), None);
        }
        plan.run();
        assert_eq!(plan.status(), Some(true));

        // test reset
        plan.exit(false);
        for _ in 0..5 {
            plan.run();
            assert_eq!(plan.status(), None);
        }
        plan.run();
        assert_eq!(plan.status(), Some(true));

        // test stop on failure
        plan.exit(false);
        for _ in 0..3 {
            plan.run();
            assert_eq!(plan.status(), None);
        }
        plan.cast_mut::<RepeatBehaviour<DC>>().unwrap().behaviour =
            Box::new(AnySuccessStatus.into());
        plan.run();
        assert_eq!(plan.status(), Some(false));

        // test retry bool
        plan.exit(false);
        plan.cast_mut::<RepeatBehaviour<DC>>().unwrap().retry = true;
        for _ in 0..3 {
            plan.run();
            assert_eq!(plan.status(), None);
        }
        plan.cast_mut::<RepeatBehaviour<DC>>().unwrap().behaviour =
            Box::new(AllSuccessStatus.into());
        plan.run();
        assert_eq!(plan.status(), Some(true));
    }

    #[test]
    fn sequence_behaviour() {
        //use tracing::info;
        //let _ = tracing_subscriber::fmt::try_init();
        let mut plan = Plan::<DC>::new(SequenceBehaviour::default().into(), "root", 1, true);
        // the first 5 child plans return success
        for i in 0..5 {
            plan.insert(Plan::new(AllSuccessStatus.into(), i.to_string(), 0, i == 0));
            plan.transitions.push(Transition {
                src: vec![i.to_string()],
                dst: vec![(i + 1).to_string()],
                predicate: predicate::True.into(),
            });
        }
        // the last child plan returns None
        plan.insert(Plan::new_stub("5", false));
        // check that child plans sequentually transition as long current child status succeeds
        for i in 0..5 {
            plan.run();
            let active = plan.plans.iter().find(|x| x.active()).unwrap().name();
            assert_eq!(active, &(i + 1).to_string());
            assert_eq!(plan.status(), None);
        }
        // check that child plans stop transitioning when current child status is None
        for _ in 0..5 {
            plan.run();
            let active = plan.plans.iter().find(|x| x.active()).unwrap().name();
            assert_eq!(active, "5");
            assert_eq!(plan.status(), None);
        }
        // change the last child plan to success as well
        plan.insert(Plan::new(AllSuccessStatus.into(), "5", 0, false));
        // expect sequence behaviour to return success when all children are successful
        plan.run();
        assert_eq!(plan.status(), Some(true));
        // expect that sequence will jump back to previusly successful child if status changes
        plan.insert(Plan::new_stub("3", false));
        plan.run();
        assert_eq!(plan.plans.iter().find(|x| x.active()).unwrap().name(), "3");
        assert_eq!(plan.status(), None);
        // same test above with failure status instead
        plan.insert(Plan::new(AnySuccessStatus.into(), "1", 0, false));
        plan.run();
        assert_eq!(plan.plans.iter().find(|x| x.active()).unwrap().name(), "1");
        assert_eq!(plan.status(), Some(false));
    }

    #[test]
    fn max_util_behaviour() {
        //use tracing::info;
        //let _ = tracing_subscriber::fmt::try_init();
        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        pub struct SetUtilBehaviour(pub f64);
        impl<C: Config> Behaviour<C> for SetUtilBehaviour {
            fn status(&self, _plan: &Plan<C>) -> Option<bool> {
                None
            }
            fn utility(&self, _plan: &Plan<C>) -> f64 {
                self.0
            }
        }

        #[enum_dispatch(Behaviour<C>)]
        #[derive(EnumCast)]
        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        pub enum TestBehaviours<C: Config> {
            EvaluateStatus(EvaluateStatus<C>),
            MaxUtilBehaviour,
            SetUtilBehaviour,
        }

        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        struct TestConfig;
        impl Config for TestConfig {
            type Predicate = predicate::Predicates;
            type Behaviour = TestBehaviours<Self>;
        }
        type TC = TestConfig;
        let mut plan = Plan::<TC>::new(MaxUtilBehaviour.into(), "root", 1, true);
        // insert 5 child plans with ascending utility
        for i in 0..5 {
            plan.insert(Plan::new(
                SetUtilBehaviour(i.into()).into(),
                i.to_string(),
                0,
                false,
            ));
        }
        // expect that highest utility plan is entered
        plan.run();
        let mut active = plan
            .plans
            .iter_mut()
            .filter(|x| x.active())
            .collect::<Vec<_>>();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name(), "4");
        // reduce utility of active plan and expect transition to another
        active[0].cast_mut::<SetUtilBehaviour>().unwrap().0 = 0.0;
        plan.run();
        let active = plan
            .plans
            .iter_mut()
            .filter(|x| x.active())
            .collect::<Vec<_>>();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name(), "3");
        // increase utility of non-active plan and expect transition to it
        plan.get_mut("2")
            .unwrap()
            .cast_mut::<SetUtilBehaviour>()
            .unwrap()
            .0 = 10.0;
        plan.run();
        let active = plan
            .plans
            .iter_mut()
            .filter(|x| x.active())
            .collect::<Vec<_>>();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name(), "2");
    }
}
