use crate::plan::Plan;
use crate::predicate::*;

use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};

#[enum_dispatch]
#[derive(Serialize, Deserialize)]
pub enum BehaviourEnum {
    DefaultBehaviour,

    EvaluateStatus,
    AllSuccessStatus,
    AnySuccessStatus,
    ModifyStatus,

    MultiBehaviour,
    SequenceBehaviour,
    FallbackBehaviour,
    MaxUtilBehaviour,
    RepeatBehaviour,

    RunCountBehaviour,
    SetStatusBehaviour,
}

#[enum_dispatch(BehaviourEnum)]
pub trait Behaviour: Send {
    fn status(&self, plan: &Plan) -> Option<bool>;
    fn utility(&self, _plan: &Plan) -> f64 {
        0.
    }
    fn on_entry(&mut self, _plan: &mut Plan) {}
    fn on_exit(&mut self, _plan: &mut Plan) {}
    fn on_run(&mut self, _plan: &mut Plan) {}
}

#[derive(Serialize, Deserialize)]
pub struct DefaultBehaviour;
impl Behaviour for DefaultBehaviour {
    fn status(&self, _plan: &Plan) -> Option<bool> {
        None
    }
}

#[derive(Serialize, Deserialize)]
pub struct EvaluateStatus(pub PredicateEnum, pub PredicateEnum);
impl Behaviour for EvaluateStatus {
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
pub struct AllSuccessStatus;
impl Behaviour for AllSuccessStatus {
    fn status(&self, plan: &Plan) -> Option<bool> {
        EvaluateStatus(AllSuccess.into(), AnyFailure.into()).status(plan)
    }
}

#[derive(Serialize, Deserialize)]
pub struct AnySuccessStatus;
impl Behaviour for AnySuccessStatus {
    fn status(&self, plan: &Plan) -> Option<bool> {
        EvaluateStatus(AnySuccess.into(), AllFailure.into()).status(plan)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ModifyStatus(pub Box<BehaviourEnum>, pub Option<bool>);
impl Behaviour for ModifyStatus {
    fn status(&self, plan: &Plan) -> Option<bool> {
        self.0.status(plan).map(|x| self.1.unwrap_or(!x))
    }
    fn utility(&self, plan: &Plan) -> f64 {
        self.0.utility(plan)
    }
    fn on_entry(&mut self, plan: &mut Plan) {
        self.0.on_entry(plan);
    }
    fn on_exit(&mut self, plan: &mut Plan) {
        self.0.on_exit(plan);
    }
    fn on_run(&mut self, plan: &mut Plan) {
        self.0.on_run(plan);
    }
}

#[derive(Serialize, Deserialize)]
pub struct RepeatBehaviour {
    pub behaviour: Box<BehaviourEnum>,
    pub condition: PredicateEnum,
    pub retry: bool,
    pub iterations: usize,
    pub count_down: usize,
    pub status: Option<bool>,
}

impl Behaviour for RepeatBehaviour {
    fn status(&self, _plan: &Plan) -> Option<bool> {
        self.status
    }
    fn utility(&self, plan: &Plan) -> f64 {
        self.behaviour.utility(plan)
    }
    fn on_entry(&mut self, plan: &mut Plan) {
        self.status = None;
        self.count_down = self.iterations;
        self.behaviour.on_entry(plan);
    }
    fn on_exit(&mut self, plan: &mut Plan) {
        self.behaviour.on_exit(plan);
    }
    fn on_run(&mut self, plan: &mut Plan) {
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
pub struct SequenceBehaviour(Vec<String>);
impl Behaviour for SequenceBehaviour {
    fn status(&self, plan: &Plan) -> Option<bool> {
        AllSuccessStatus.status(plan)
    }
    fn on_run(&mut self, plan: &mut Plan) {
        check_visited_status_and_jump(&mut self.0, plan);
    }
}

#[derive(Serialize, Deserialize)]
pub struct FallbackBehaviour(Vec<String>);
impl Behaviour for FallbackBehaviour {
    fn status(&self, plan: &Plan) -> Option<bool> {
        AnySuccessStatus.status(plan)
    }
    fn on_run(&mut self, plan: &mut Plan) {
        check_visited_status_and_jump(&mut self.0, plan);
    }
}

fn check_visited_status_and_jump(visited: &mut Vec<String>, plan: &mut Plan) {
    // find first inactive visited plans that have status none
    let pos = visited.iter().position(|x| match plan.get(x) {
        Some(x) => !x.active() && x.behaviour.status(plan).is_none(),
        None => false,
    });
    // jump back to that plan
    if let Some(pos) = pos {
        plan.exit_all(None);
        plan.enter(&visited[pos]);
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
impl Behaviour for MaxUtilBehaviour {
    fn status(&self, plan: &Plan) -> Option<bool> {
        AnySuccessStatus.status(plan)
    }
    fn utility(&self, plan: &Plan) -> f64 {
        match max_utility(&plan.plans) {
            Some((_, util)) => util,
            None => 0.,
        }
    }
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
    fn on_run(&mut self, plan: &mut Plan) {
        for behaviour in &mut self.0 {
            behaviour.on_run(plan);
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct RunCountBehaviour {
    pub entry_count: u32,
    pub exit_count: u32,
    pub run_count: u32,
}

impl Behaviour for RunCountBehaviour {
    fn status(&self, _plan: &Plan) -> Option<bool> {
        None
    }
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
