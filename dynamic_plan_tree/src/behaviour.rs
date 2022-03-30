use crate::{predicate::Predicates, *};

/// Macro to redefine `Behaviour` trait in external crates for remote enum_dispatch definition.
#[macro_export]
macro_rules! behaviour_trait {
    () => {
        /// An object that implements runtime behaviour logic of an active plan.
        #[enum_dispatch]
        pub trait Behaviour: Send {
            fn status(&self, plan: &Plan<impl Config>) -> Option<bool>;
            fn utility(&self, _plan: &Plan<impl Config>) -> f64 {
                0.
            }
            fn on_entry(&mut self, _plan: &mut Plan<impl Config>) {}
            fn on_exit(&mut self, _plan: &mut Plan<impl Config>) {}
            fn on_run(&mut self, _plan: &mut Plan<impl Config>) {}
        }
    };
}
behaviour_trait!();

/// Default set of built-in behaviours to serve as example template.
#[enum_dispatch(Behaviour)]
#[derive(Serialize, Deserialize)]
pub enum Behaviours {
    Default,
    EvaluateStatus(EvaluateStatus<Predicates, Predicates>),
    AllSuccessStatus,
    AnySuccessStatus,
    ModifyStatus(ModifyStatus<Self>),

    Multi(Multi<Self>),
    Repeat(Repeat<Self, Predicates>),
    Sequence,
    Fallback,
    MaxUtil,
}

#[derive(Serialize, Deserialize)]
pub struct Default;
impl Behaviour for Default {
    fn status(&self, _plan: &Plan<impl Config>) -> Option<bool> {
        None
    }
}

#[derive(Serialize, Deserialize)]
pub struct EvaluateStatus<T, F>(pub T, pub F);
impl<T: Predicate, F: Predicate> Behaviour for EvaluateStatus<T, F> {
    fn status(&self, plan: &Plan<impl Config>) -> Option<bool> {
        if self.1.evaluate(plan, &[]) {
            Some(false)
        } else if self.0.evaluate(plan, &[]) {
            Some(true)
        } else {
            None
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct AllSuccessStatus;
impl Behaviour for AllSuccessStatus {
    fn status(&self, plan: &Plan<impl Config>) -> Option<bool> {
        EvaluateStatus(predicate::AllSuccess, predicate::AnyFailure).status(plan)
    }
}

#[derive(Serialize, Deserialize)]
pub struct AnySuccessStatus;
impl Behaviour for AnySuccessStatus {
    fn status(&self, plan: &Plan<impl Config>) -> Option<bool> {
        EvaluateStatus(predicate::AnySuccess, predicate::AllFailure).status(plan)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ModifyStatus<B>(pub Box<B>, pub Option<bool>);
impl<B: Behaviour> Behaviour for ModifyStatus<B> {
    fn status(&self, plan: &Plan<impl Config>) -> Option<bool> {
        self.0.status(plan).map(|x| self.1.unwrap_or(!x))
    }
    fn utility(&self, plan: &Plan<impl Config>) -> f64 {
        self.0.utility(plan)
    }
    fn on_entry(&mut self, plan: &mut Plan<impl Config>) {
        self.0.on_entry(plan);
    }
    fn on_exit(&mut self, plan: &mut Plan<impl Config>) {
        self.0.on_exit(plan);
    }
    fn on_run(&mut self, plan: &mut Plan<impl Config>) {
        self.0.on_run(plan);
    }
}

#[derive(Serialize, Deserialize)]
pub struct Repeat<B, P> {
    pub behaviour: Box<B>,
    pub condition: P,
    pub retry: bool,
    pub iterations: usize,
    pub count_down: usize,
    pub status: Option<bool>,
}

impl<B: Behaviour, P: Predicate> Behaviour for Repeat<B, P> {
    fn status(&self, _plan: &Plan<impl Config>) -> Option<bool> {
        self.status
    }
    fn utility(&self, plan: &Plan<impl Config>) -> f64 {
        self.behaviour.utility(plan)
    }
    fn on_entry(&mut self, plan: &mut Plan<impl Config>) {
        self.status = None;
        self.count_down = self.iterations;
        self.behaviour.on_entry(plan);
    }
    fn on_exit(&mut self, plan: &mut Plan<impl Config>) {
        self.behaviour.on_exit(plan);
    }
    fn on_run(&mut self, plan: &mut Plan<impl Config>) {
        if self.status.is_some() {
            return;
        }
        if self.count_down == 0 || !self.condition.evaluate(plan, &[]) {
            self.status = Some(!self.retry);
            return;
        }
        self.behaviour.on_run(plan);
        if let Some(success) = self.behaviour.status(plan) {
            if success != self.retry {
                // if success, decrement countdown and reset behaviour
                self.count_down -= 1;
                self.behaviour.on_exit(plan);
                self.behaviour.on_entry(plan);
            } else {
                // if failure, store status and stop
                self.status = Some(self.retry);
            }
        }
        // otherwise keep running behaviour
    }
}

#[derive(Serialize, Deserialize)]
pub struct Sequence(Vec<String>);
impl Behaviour for Sequence {
    fn status(&self, plan: &Plan<impl Config>) -> Option<bool> {
        AllSuccessStatus.status(plan)
    }
    fn on_run(&mut self, plan: &mut Plan<impl Config>) {
        check_visited_status_and_jump(&mut self.0, plan);
    }
}

#[derive(Serialize, Deserialize)]
pub struct Fallback(Vec<String>);
impl Behaviour for Fallback {
    fn status(&self, plan: &Plan<impl Config>) -> Option<bool> {
        AnySuccessStatus.status(plan)
    }
    fn on_run(&mut self, plan: &mut Plan<impl Config>) {
        check_visited_status_and_jump(&mut self.0, plan);
    }
}

fn check_visited_status_and_jump(visited: &mut Vec<String>, plan: &mut Plan<impl Config>) {
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
pub struct MaxUtil;
impl Behaviour for MaxUtil {
    fn status(&self, plan: &Plan<impl Config>) -> Option<bool> {
        AnySuccessStatus.status(plan)
    }
    fn utility(&self, plan: &Plan<impl Config>) -> f64 {
        match max_utility(&plan.plans) {
            Some((_, util)) => util,
            None => 0.,
        }
    }
    fn on_run(&mut self, plan: &mut Plan<impl Config>) {
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

pub fn max_utility(plans: &Vec<Plan<impl Config>>) -> Option<(&Plan<impl Config>, f64)> {
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

#[derive(Serialize, Deserialize)]
pub struct Multi<B>(pub Vec<B>);
impl<B: Behaviour> Behaviour for Multi<B> {
    fn status(&self, plan: &Plan<impl Config>) -> Option<bool> {
        let mut status = Some(true);
        for behaviour in &self.0 {
            match behaviour.status(&plan) {
                Some(true) => {}
                Some(false) => return Some(false),
                None => status = None,
            }
        }
        status
    }
    fn utility(&self, plan: &Plan<impl Config>) -> f64 {
        self.0.iter().map(|behaviour| behaviour.utility(plan)).sum()
    }
    fn on_entry(&mut self, plan: &mut Plan<impl Config>) {
        for behaviour in &mut self.0 {
            behaviour.on_entry(plan);
        }
    }
    fn on_exit(&mut self, plan: &mut Plan<impl Config>) {
        for behaviour in self.0.iter_mut().rev() {
            behaviour.on_exit(plan);
        }
    }
    fn on_run(&mut self, plan: &mut Plan<impl Config>) {
        for behaviour in &mut self.0 {
            behaviour.on_run(plan);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::predicate::*;
    use tracing::debug;

    #[derive(Serialize, Deserialize)]
    struct TestConfig;
    impl Config for TestConfig {
        type Predicate = Predicates;
        type Behaviour = Behaviours;
    }

    #[test]
    fn generate_schema() {
        let _ = tracing_subscriber::fmt::try_init();
        // generate and print plan schema
        use serde_reflection::{Tracer, TracerConfig};
        let mut tracer = Tracer::new(TracerConfig::default());
        tracer.trace_simple_type::<Behaviours>().unwrap();
        tracer.trace_simple_type::<Predicates>().unwrap();
        let registry = tracer.registry().unwrap();
        debug!("{}", serde_json::to_string_pretty(&registry).unwrap());
    }

    #[test]
    fn generate_plan() {
        use std::time::Duration;
        let _ = tracing_subscriber::fmt::try_init();
        let root_plan = Plan::<TestConfig>::new(Default.into(), "root", true, Duration::new(0, 0));
        // serialize and print root plan
        debug!("{}", serde_json::to_string_pretty(&root_plan).unwrap());
    }
}
