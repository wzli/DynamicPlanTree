use crate::*;

#[typetag::serde(tag = "type")]
pub trait Behaviour: Send + Downcast {
    // required
    fn on_run(&mut self, _plan: &mut Plan);
    // optional
    fn on_entry(&mut self, _plan: &mut Plan) {}
    fn on_exit(&mut self, _plan: &mut Plan) {}
    fn utility(&mut self, _plan: &mut Plan) -> f64 {
        0.
    }
}
impl_downcast!(Behaviour);

#[derive(Serialize, Deserialize)]
pub struct DefaultBehaviour;
#[typetag::serde]
impl Behaviour for DefaultBehaviour {
    fn on_run(&mut self, _plan: &mut Plan) {}
}

#[derive(Serialize, Deserialize)]
pub struct MultiBehaviour(Vec<Box<dyn Behaviour>>);
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
    fn utility(&mut self, plan: &mut Plan) -> f64 {
        self.0
            .iter_mut()
            .map(|behaviour| behaviour.utility(plan))
            .sum()
    }
}

#[derive(Serialize, Deserialize)]
pub struct EvalBehaviour(Box<dyn Predicate>, Box<dyn Predicate>);
#[typetag::serde]
impl Behaviour for EvalBehaviour {
    fn on_run(&mut self, plan: &mut Plan) {
        plan.status = if self.1.evaluate(plan, &HashSet::new()) {
            Some(false)
        } else if self.0.evaluate(plan, &HashSet::new()) {
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
    fn on_run(&mut self, plan: &mut Plan) {
        plan.status = if AnyFailure.evaluate(plan, &HashSet::new()) {
            Some(false)
        } else if AllSuccess.evaluate(plan, &HashSet::new()) {
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
    fn on_run(&mut self, plan: &mut Plan) {
        plan.status = if AllFailure.evaluate(plan, &HashSet::new()) {
            Some(false)
        } else if AnySuccess.evaluate(plan, &HashSet::new()) {
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
        let best = match max_utility(&mut plan.plans) {
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
    fn utility(&mut self, plan: &mut Plan) -> f64 {
        match max_utility(&mut plan.plans) {
            Some((_, util)) => util,
            None => 0.,
        }
    }
}

pub fn max_utility(plans: &mut Vec<Plan>) -> Option<(&Plan, f64)> {
    if plans.is_empty() {
        None
    } else {
        let (pos, utility) = plans
            .par_iter_mut()
            .map(|plan| plan.call("utility", |behaviour, plan| behaviour.utility(plan)))
            .enumerate()
            .collect::<Vec<_>>()
            .into_iter()
            .fold((0, f64::NAN), |max, x| if max.1 > x.1 { max } else { x });
        Some((&plans[pos], utility))
    }
}
