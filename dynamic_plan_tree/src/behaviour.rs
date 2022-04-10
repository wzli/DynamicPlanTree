pub use crate::*;

/// Macro to redefine `Behaviour` trait in external crates for remote enum_dispatch definition.
#[macro_export]
macro_rules! behaviour_trait {
    () => {
        /// An object that implements runtime behaviour logic of an active plan.
        #[enum_dispatch]
        pub trait Behaviour<C: Config>: Sized + 'static {
            fn status(&self, plan: &Plan<C>) -> Option<bool>;
            fn utility(&self, _plan: &Plan<C>) -> f64 {
                0.
            }
            fn on_entry(&mut self, _plan: &mut Plan<C>) {}
            fn on_exit(&mut self, _plan: &mut Plan<C>) {}

            fn on_pre_run(&mut self, _plan: &mut Plan<C>) {}
            fn on_run(&mut self, _plan: &mut Plan<C>) {}

            fn as_any(&self) -> &dyn Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn Any {
                self
            }
        }
    };
}
behaviour_trait!();

/// Default set of built-in behaviours to serve as example template.
#[enum_dispatch(Behaviour<C>)]
#[derive(Serialize, Deserialize, FromAny)]
pub enum Behaviours<C: Config> {
    DefaultBehaviour,

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

/// Empty behaviour used as default placeholder. Must be included in custom behaviour sets.
#[derive(Serialize, Deserialize)]
pub struct DefaultBehaviour;
impl<C: Config> Behaviour<C> for DefaultBehaviour {
    fn status(&self, _plan: &Plan<C>) -> Option<bool> {
        None
    }
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
#[derive(Serialize, Deserialize)]
pub struct EvaluateStatus<C: Config>(pub C::Predicate, pub C::Predicate);
impl<C: Config> Behaviour<C> for EvaluateStatus<C> {
    fn status(&self, plan: &Plan<C>) -> Option<bool> {
        evaluate_status(plan, &self.0, &self.1)
    }
}

/// Behaviour with status `true` if `AllSuccess`, `false` if `AnyFailure`, otherwise `None`.
#[derive(Serialize, Deserialize)]
pub struct AllSuccessStatus;
impl<C: Config> Behaviour<C> for AllSuccessStatus {
    fn status(&self, plan: &Plan<C>) -> Option<bool> {
        evaluate_status(plan, &predicate::AllSuccess, &predicate::AnyFailure)
    }
}

/// Behaviour with status `true` if `AnySuccess`, `false` if `AllFailure`, otherwise `None`.
#[derive(Serialize, Deserialize)]
pub struct AnySuccessStatus;
impl<C: Config> Behaviour<C> for AnySuccessStatus {
    fn status(&self, plan: &Plan<C>) -> Option<bool> {
        evaluate_status(plan, &predicate::AnySuccess, &predicate::AllFailure)
    }
}

/// Wraps inner behaviour. If inner status exists, invert when `self.1` is `None` otherwise use `self.1`.
#[derive(Serialize, Deserialize)]
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
    fn on_pre_run(&mut self, plan: &mut Plan<C>) {
        self.0.on_pre_run(plan);
    }
    fn on_run(&mut self, plan: &mut Plan<C>) {
        self.0.on_run(plan);
    }
}

/// Vector of behaviours sharing the same plan. Status takes aggregate AND. Utility takes aggregate sum.
#[derive(Serialize, Deserialize)]
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
    fn on_pre_run(&mut self, plan: &mut Plan<C>) {
        for behaviour in &mut self.0 {
            behaviour.on_pre_run(plan);
        }
    }
    fn on_run(&mut self, plan: &mut Plan<C>) {
        for behaviour in &mut self.0 {
            behaviour.on_run(plan);
        }
    }
}

/// Repeats inner behaviour for specified iterations, until failure encountered, while provided condition holds.
#[derive(Serialize, Deserialize)]
pub struct RepeatBehaviour<C: Config> {
    /// Behaviour that expects some status on completion to mark each iteration.
    pub behaviour: Box<C::Behaviour>,
    /// Stop running behaviour once condition no longer holds.
    pub condition: C::Predicate,
    /// Stop running behaviour after specified iterations.
    pub iterations: usize,
    /// Repeat until behaviour status returns either success or failure.
    pub retry: bool,

    count_down: usize,
    status: Option<bool>,
}

impl<C: Config> RepeatBehaviour<C> {
    pub fn new(behaviour: impl Into<Box<C::Behaviour>>) -> Self {
        Self {
            behaviour: behaviour.into(),
            condition: C::Predicate::from_any(predicate::True).unwrap(),
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
    fn on_pre_run(&mut self, plan: &mut Plan<C>) {
        // run only while status is indeterminant
        if self.status.is_some() {
            return;
        }
        // stop when countdown runs out or condition doesn't hold
        if self.count_down == 0 || !self.condition.evaluate(plan, &[]) {
            self.status = Some(!self.retry);
            return;
        }
        self.behaviour.on_pre_run(plan);
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

#[derive(Serialize, Deserialize)]
pub struct SequenceBehaviour(Vec<String>);
impl<C: Config> Behaviour<C> for SequenceBehaviour {
    fn status(&self, plan: &Plan<C>) -> Option<bool> {
        AllSuccessStatus.status(plan)
    }
    fn on_pre_run(&mut self, plan: &mut Plan<C>) {
        check_visited_status_and_jump(&mut self.0, plan);
    }
}

#[derive(Serialize, Deserialize)]
pub struct FallbackBehaviour(Vec<String>);
impl<C: Config> Behaviour<C> for FallbackBehaviour {
    fn status(&self, plan: &Plan<C>) -> Option<bool> {
        AnySuccessStatus.status(plan)
    }
    fn on_pre_run(&mut self, plan: &mut Plan<C>) {
        check_visited_status_and_jump(&mut self.0, plan);
    }
}

fn check_visited_status_and_jump<C: Config>(visited: &mut Vec<String>, plan: &mut Plan<C>) {
    // find first inactive visited plans that have status none
    let pos = visited.iter().position(|x| match plan.get(x) {
        Some(x) => !x.active() && x.behaviour.status(plan).is_none(),
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
    match visited.last() {
        Some(last) if last == active => return,
        _ => {}
    }
    visited.push(active.clone());
}

#[derive(Serialize, Deserialize)]
pub struct MaxUtilBehaviour;
impl<C: Config> Behaviour<C> for MaxUtilBehaviour {
    fn status(&self, plan: &Plan<C>) -> Option<bool> {
        AnySuccessStatus.status(plan)
    }
    fn utility(&self, plan: &Plan<C>) -> f64 {
        match max_utility(&plan.plans) {
            Some((_, util)) => util,
            None => 0.,
        }
    }
    fn on_pre_run(&mut self, plan: &mut Plan<C>) {
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

/// Find and return the plan with higest utility.
pub fn max_utility<C: Config>(plans: &[Plan<C>]) -> Option<(&Plan<C>, f64)> {
    if plans.is_empty() {
        None
    } else {
        let (pos, utility) = plans
            .iter()
            .map(|plan| plan.behaviour.utility(plan))
            .enumerate()
            .fold((0, f64::NAN), |max, x| if max.1 > x.1 { max } else { x });
        Some((&plans[pos], utility))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use tracing::debug;

    #[derive(Serialize, Deserialize)]
    struct DefaultConfig;
    impl Config for DefaultConfig {
        type Predicate = predicate::Predicates;
        type Behaviour = behaviour::Behaviours<Self>;
    }

    #[test]
    fn evaluate_status() {
        let make_plan = |t: bool, f: bool| {
            Plan::<DefaultConfig>::new(
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
        assert_eq!(plan.behaviour.status(&plan), None);

        let plan = make_plan(false, true);
        assert_eq!(plan.behaviour.status(&plan), Some(false));

        let plan = make_plan(true, false);
        assert_eq!(plan.behaviour.status(&plan), Some(true));

        let plan = make_plan(true, true);
        assert_eq!(plan.behaviour.status(&plan), Some(false));
    }

    #[test]
    fn all_success_status() {}
}
