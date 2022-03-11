use crate::*;

#[typetag::serde(tag = "type")]
pub trait Behaviour: Send {
    // required
    fn as_any(&self) -> &dyn Any;
    fn on_run(&mut self, _plan: &mut Plan);
    // optional
    fn on_entry(&mut self, _plan: &mut Plan) {}
    fn on_exit(&mut self, _plan: &mut Plan) {}
    fn utility(&mut self, _plan: &mut Plan) -> f64 {
        0.
    }
}

pub fn cast<T: Behaviour + 'static>(sm: &dyn Behaviour) -> Option<&T> {
    sm.as_any().downcast_ref::<T>()
}

#[derive(Serialize, Deserialize)]
pub struct DefaultBehaviour;
#[typetag::serde]
impl Behaviour for DefaultBehaviour {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn on_run(&mut self, _plan: &mut Plan) {}
}

#[derive(Serialize, Deserialize)]
pub struct MultiBehaviour(Vec<Box<dyn Behaviour>>);
#[typetag::serde]
impl Behaviour for MultiBehaviour {
    fn as_any(&self) -> &dyn Any {
        self
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
pub struct EvalBehaviour(Box<dyn Predicate>, bool);
#[typetag::serde]
impl Behaviour for EvalBehaviour {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn on_run(&mut self, plan: &mut Plan) {
        if plan.status.is_some() {
            return;
        } else if self.0.evaluate(plan, &HashSet::new()) {
            plan.status = Some(self.1);
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct MaxUtilBehaviour;
#[typetag::serde]
impl Behaviour for MaxUtilBehaviour {
    fn as_any(&self) -> &dyn Any {
        self
    }
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