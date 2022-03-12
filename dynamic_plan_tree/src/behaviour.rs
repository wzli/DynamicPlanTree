use crate::*;

#[typetag::serde(tag = "type")]
pub trait Behaviour: Send + Downcast {
    fn status(&self, _plan: &Plan) -> Option<bool> {
        None
    }
    fn utility(&self, _plan: &Plan) -> f64 {
        0.
    }
    fn on_run(&mut self, _plan: &mut Plan) {}
    fn on_entry(&mut self, _plan: &mut Plan) {}
    fn on_exit(&mut self, _plan: &mut Plan) {}
}
impl_downcast!(Behaviour);

#[derive(Serialize, Deserialize)]
pub struct DefaultBehaviour;
#[typetag::serde]
impl Behaviour for DefaultBehaviour {}

#[derive(Serialize, Deserialize)]
pub struct MultiBehaviour(pub Vec<Box<dyn Behaviour>>);
#[typetag::serde]
impl Behaviour for MultiBehaviour {
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
            behaviour.on_entry(plan);
        }
    }
    fn utility(&self, plan: &Plan) -> f64 {
        self.0.iter().map(|behaviour| behaviour.utility(plan)).sum()
    }
}

#[derive(Serialize, Deserialize)]
pub struct EvalBehaviour(pub Box<dyn Predicate>, pub Box<dyn Predicate>);
#[typetag::serde]
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
#[typetag::serde]
impl Behaviour for SequenceBehaviour {
    fn status(&self, plan: &Plan) -> Option<bool> {
        if AnyFailure.evaluate(plan, &[]) {
            Some(false)
        } else if AllSuccess.evaluate(plan, &[]) {
            Some(true)
        } else {
            None
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct FallbackBehaviour;
#[typetag::serde]
impl Behaviour for FallbackBehaviour {
    fn status(&self, plan: &Plan) -> Option<bool> {
        if AllFailure.evaluate(plan, &[]) {
            Some(false)
        } else if AnySuccess.evaluate(plan, &[]) {
            Some(true)
        } else {
            None
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct MaxUtilBehaviour;
#[typetag::serde]
impl Behaviour for MaxUtilBehaviour {
    fn on_run(&mut self, plan: &mut Plan) {
        // get highest utility plan
        let best = match max_utility(&plan.plans) {
            Some((plan, _)) => plan.name().clone(),
            None => return,
        };
        // get active plan
        if let Some(Plan { name: active, .. }) = plan.plans.iter().find(|plan| plan.active) {
            // current plan is already best
            if *active == best {
                return;
            }
            // exit active plan
            let active = active.clone();
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
