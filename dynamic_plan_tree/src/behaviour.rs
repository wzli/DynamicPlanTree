use crate::plan::Plan;
use crate::predicate::*;

use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};

#[enum_dispatch]
#[derive(Serialize, Deserialize)]
pub enum BehaviourEnum {
    DefaultBehaviour,
    MultiBehaviour,
    EvalBehaviour,
    SequenceBehaviour,
    FallbackBehaviour,
    MaxUtilBehaviour,

    RunCountBehaviour,
    SetStatusBehaviour,
}

#[enum_dispatch(BehaviourEnum)]
pub trait Behaviour: Send {
    fn status(&self, _plan: &Plan) -> Option<bool> {
        None
    }
    fn utility(&self, _plan: &Plan) -> f64 {
        0.
    }
    fn on_entry(&mut self, _plan: &mut Plan) {}
    fn on_exit(&mut self, _plan: &mut Plan) {}
    fn on_run(&mut self, _plan: &mut Plan) {}
}

#[derive(Serialize, Deserialize)]
pub struct DefaultBehaviour;
impl Behaviour for DefaultBehaviour {}

#[derive(Serialize, Deserialize)]
pub struct MultiBehaviour(pub Vec<BehaviourEnum>);
impl Behaviour for MultiBehaviour {
    fn status(&self, plan: &Plan) -> Option<bool> {
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
    fn utility(&self, plan: &Plan) -> f64 {
        self.0.iter().map(|behaviour| behaviour.utility(plan)).sum()
    }
    fn on_run(&mut self, plan: &mut Plan) {
        for behaviour in &mut self.0 {
            behaviour.on_run(plan);
        }
    }
    fn on_entry(&mut self, plan: &mut Plan) {
        for behaviour in &mut self.0 {
            behaviour.on_entry(plan);
        }
    }
    fn on_exit(&mut self, plan: &mut Plan) {
        for behaviour in self.0.iter_mut().rev() {
            behaviour.on_exit(plan);
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct EvalBehaviour(pub PredicateEnum, pub PredicateEnum);
impl Behaviour for EvalBehaviour {
    fn status(&self, plan: &Plan) -> Option<bool> {
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
pub struct SequenceBehaviour;
impl Behaviour for SequenceBehaviour {
    fn status(&self, plan: &Plan) -> Option<bool> {
        EvalBehaviour(AllSuccess.into(), AnyFailure.into()).status(plan)
    }
}

#[derive(Serialize, Deserialize)]
pub struct FallbackBehaviour;
impl Behaviour for FallbackBehaviour {
    fn status(&self, plan: &Plan) -> Option<bool> {
        EvalBehaviour(AnySuccess.into(), AllFailure.into()).status(plan)
    }
}

#[derive(Serialize, Deserialize)]
pub struct MaxUtilBehaviour;
impl Behaviour for MaxUtilBehaviour {
    fn on_run(&mut self, plan: &mut Plan) {
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
            plan.exit(&active);
        }
        // enter new plan
        plan.enter(&best);
    }
    fn utility(&self, plan: &Plan) -> f64 {
        match max_utility(&plan.plans) {
            Some((_, util)) => util,
            None => 0.,
        }
    }
}

pub fn max_utility(plans: &Vec<Plan>) -> Option<(&Plan, f64)> {
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
pub struct TestBehaviour(pub Option<bool>);
impl Behaviour for TestBehaviour {
    fn status(&self, _: &Plan) -> Option<bool> {
        self.0
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct RunCountBehaviour {
    pub entry_count: u32,
    pub exit_count: u32,
    pub run_count: u32,
}

impl Behaviour for RunCountBehaviour {
    fn on_entry(&mut self, _plan: &mut Plan) {
        self.entry_count += 1;
    }
    fn on_exit(&mut self, _plan: &mut Plan) {
        self.exit_count += 1;
    }
    fn on_run(&mut self, _plan: &mut Plan) {
        self.run_count += 1;
    }
}

#[derive(Serialize, Deserialize)]
pub struct SetStatusBehaviour(pub Option<bool>);
impl Behaviour for SetStatusBehaviour {
    fn status(&self, _: &Plan) -> Option<bool> {
        self.0
    }
}
