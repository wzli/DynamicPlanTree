use crate::*;

#[typetag::serde(tag = "type")]
pub trait Behaviour: Send {
    // required
    fn as_any(&self) -> &dyn Any;
    fn on_run(&mut self, _plan: &mut Plan);
    // optional
    fn on_entry(&mut self, _plan: &mut Plan) {}
    fn on_exit(&mut self, _plan: &mut Plan) {}
    fn utility(&mut self, plan: &mut Plan, filter_active: bool) -> f64 {
        plan.plans
            .iter_mut()
            .filter(|plan| !filter_active || plan.active)
            .par_bridge()
            .map(|plan| {
                plan.call("utility", |behaviour, plan| {
                    behaviour.utility(plan, filter_active)
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .fold(f64::NAN, f64::max)
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
    fn on_run(&mut self, plan: &mut Plan) {}
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
}

#[derive(Serialize, Deserialize)]
pub struct EvalBehaviour(Box<dyn Predicate>, bool);
#[typetag::serde]
impl Behaviour for EvalBehaviour {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn on_run(&mut self, plan: &mut Plan) {
        if self.0.evaluate(plan, &HashSet::new()) {
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
        let best = match plan.highest_utility() {
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
}
