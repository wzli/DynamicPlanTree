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
    fn on_run(&mut self, plan: &mut Plan) {
        /*
        // get highest utility plan
        let best = match plan.highest_utility() {
            Some((plan, _)) => plan.name().clone(),
            None => return,
        };
        // get current plan
        if let Some(cur) = &plan.status {
            // current plan is already best
            if *cur == best {
                return;
            }
            // exit previous plan
            let cur = cur.clone();
            plan.exit(&cur);
        }
        // enter new plan
        plan.enter(&best);
        plan.status = Some(best);
        */
    }
}
