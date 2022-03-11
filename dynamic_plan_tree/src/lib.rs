mod predicate;

use predicate::*;

use log::debug;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::{
    any::Any,
    collections::HashSet,
    time::{Duration, Instant},
};

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
pub struct Transition {
    pub src: Vec<String>,
    pub dst: Vec<String>,
    pub predicate: Box<dyn Predicate>,
}

#[derive(Serialize, Deserialize)]
pub struct Plan {
    name: String,
    pub behaviour: Box<dyn Behaviour>,
    active: bool,
    pub status: Option<String>,
    #[serde(with = "serde_millis")]
    pub run_interval: Duration,
    pub transitions: Vec<Transition>,
    pub plans: Vec<Plan>,
    pub data: Value,
    #[serde(with = "serde_millis")]
    timestamp: Instant,
}

#[derive(Serialize, Deserialize)]
pub struct DefaultBehaviour;
#[typetag::serde]
impl Behaviour for DefaultBehaviour {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn on_run(&mut self, plan: &mut Plan) {
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
    }
}

impl Plan {
    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn timestamp(&self) -> Instant {
        self.timestamp
    }

    pub fn new<S: Into<String>>(
        behaviour: Box<dyn Behaviour>,
        name: S,
        active: bool,
        run_interval: Duration,
    ) -> Self {
        let mut plan = Plan {
            name: name.into(),
            behaviour,
            run_interval,
            status: None,
            active,
            transitions: Vec::new(),
            plans: Vec::new(),
            data: Value::Null,
            timestamp: Instant::now(),
        };
        if active {
            plan.call("on_entry", |behaviour, plan| behaviour.on_entry(plan));
        }
        plan
    }

    pub fn insert(&mut self, plan: Plan) -> &mut Plan {
        // sorted insert
        let found = self.find(&plan.name);
        let pos = found.unwrap_or_else(|x| x);
        debug!(
            "{:?}\t{}\t>> insert\t{} ->\t{:?}",
            self as *const _,
            self.name,
            plan.name,
            self.plans.iter().map(|plan| &plan.name).collect::<Vec<_>>()
        );
        match found {
            // overwrite if there is already one
            Ok(_) => self.plans[pos] = plan,
            Err(_) => self.plans.insert(pos, plan),
        }
        &mut self.plans[pos]
    }

    pub fn remove(&mut self, name: &str) -> Option<Plan> {
        let pos = self.find(name).ok()?;
        debug!(
            "{:?}\t{}\t>> remove\t{} <-\t{:?}",
            self as *const _,
            self.name,
            name,
            self.plans.iter().map(|plan| &plan.name).collect::<Vec<_>>()
        );
        Some(self.plans.remove(pos))
    }

    pub fn find(&self, name: &str) -> Result<usize, usize> {
        self.plans.binary_search_by(|plan| (*plan.name).cmp(name))
    }

    pub fn get(&self, name: &str) -> Option<&Plan> {
        let pos = self.find(name).ok()?;
        Some(&self.plans[pos])
    }

    pub fn run(&mut self) {
        // call run() recursively
        self.plans
            .iter_mut()
            .filter(|plan| plan.active)
            .par_bridge()
            .for_each(|plan| plan.run());

        // limit execution frequency
        if self.timestamp.elapsed() < self.run_interval {
            return;
        }
        self.timestamp = Instant::now();

        // get active set of plans
        let active_plans = self
            .plans
            .iter()
            .filter(|plan| plan.active)
            .map(|plan| plan.name.clone())
            .collect::<HashSet<_>>();

        // evaluate state transitions
        let transitions = std::mem::take(&mut self.transitions);
        transitions
            .iter()
            .filter(|t| {
                t.src.len() == active_plans.len()
                    && t.src.iter().all(|p| active_plans.contains(p))
                    && t.predicate.evaluate(self)
            })
            .collect::<Vec<_>>()
            .iter()
            .for_each(|t| {
                t.src.iter().filter(|p| !t.dst.contains(p)).for_each(|p| {
                    self.exit(p);
                });
                t.dst.iter().filter(|p| !t.src.contains(p)).for_each(|p| {
                    self.enter(p);
                });
            });
        let _ = std::mem::replace(&mut self.transitions, transitions);

        // run the state machine of this plan
        self.call("on_run", |behaviour, plan| behaviour.on_run(plan));
    }

    pub fn enter(&mut self, name: &str) -> Option<&mut Plan> {
        // can only enter plans within an active plan
        if !self.active {
            return None;
        }
        // look for requested plan
        match self.find(name) {
            Ok(pos) => {
                let plan = &mut self.plans[pos];
                // if plan is inactive, set as active and call on_entry()
                if !plan.active {
                    plan.active = true;
                    plan.call("on_entry", |behaviour, plan| behaviour.on_entry(plan));
                }
                Some(plan)
            }
            // if plan doesn't exist, create and insert an default plan
            Err(pos) => {
                let default =
                    Plan::new(Box::new(DefaultBehaviour), name, true, Duration::new(0, 0));
                self.plans.insert(pos, default);
                Some(&mut self.plans[pos])
            }
        }
    }

    pub fn exit(&mut self, name: &str) -> Option<&mut Plan> {
        // ignore if plan is not found
        let pos = self.find(name).ok()?;
        let plan = &mut self.plans[pos];
        // only exit if plan is active
        if plan.active {
            plan.exit_all();
        }
        Some(plan)
    }

    pub fn exit_all(&mut self) {
        // recursively exit all active child plans
        self.plans
            .iter_mut()
            .filter(|plan| plan.active)
            .par_bridge()
            .for_each(|plan| plan.exit_all());

        // trigger on_exit() for self
        self.active = false;
        self.call("on_exit", |behaviour, plan| behaviour.on_exit(plan));
    }

    pub fn highest_utility(&mut self) -> Option<(&Plan, f64)> {
        if self.plans.is_empty() {
            None
        } else {
            let (pos, utility) = self
                .plans
                .par_iter_mut()
                .map(|plan| plan.call("utility", |behaviour, plan| behaviour.utility(plan, false)))
                .enumerate()
                .collect::<Vec<_>>()
                .into_iter()
                .fold((0, f64::NAN), |max, x| if max.1 > x.1 { max } else { x });
            Some((&self.plans[pos], utility))
        }
    }

    pub fn debug_log(&self, pre: &str, tag: &str) {
        debug!(
            "{:?}\t{}\t{} {}\t{:?}\t{:?}",
            self as *const _,
            self.name,
            pre,
            tag,
            self.status,
            self.plans
                .iter()
                .filter(|plan| plan.active)
                .map(|plan| &plan.name)
                .collect::<Vec<_>>()
        );
    }

    fn call<F, T>(&mut self, f_name: &str, f: F) -> T
    where
        F: FnOnce(&mut Box<dyn Behaviour>, &mut Plan) -> T,
    {
        self.debug_log(">>", f_name);
        let mut behaviour = std::mem::replace(&mut self.behaviour, Box::new(DefaultBehaviour));
        let ret = f(&mut behaviour, self);
        let _ = std::mem::replace(&mut self.behaviour, behaviour);
        self.debug_log("<<", f_name);
        ret
    }
}

impl Drop for Plan {
    fn drop(&mut self) {
        if self.active {
            self.active = false;
            self.call("on_exit", |behaviour, plan| behaviour.on_exit(plan));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[derive(Serialize, Deserialize, Default)]
    struct TestBehaviour {
        entry_count: u32,
        exit_count: u32,
        run_count: u32,
    }

    #[typetag::serde]
    impl Behaviour for TestBehaviour {
        fn as_any(&self) -> &dyn Any {
            self
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

    fn new_plan(name: &str, active: bool) -> Plan {
        Plan::new(
            Box::new(TestBehaviour::default()),
            name,
            active,
            Duration::new(0, 0),
        )
    }

    #[test]
    fn sorted_insert() {
        let _ = env_logger::try_init();

        let mut root_plan = new_plan("root", true);
        root_plan.insert(new_plan("C", true));
        root_plan.insert(new_plan("A", true));
        root_plan.insert(new_plan("B", true));
        root_plan.insert(new_plan("B", true));

        assert_eq!(root_plan.plans.len(), 3);
        for (i, plan) in root_plan.plans.iter().enumerate() {
            assert!(plan.active());
            assert_eq!(plan.name(), &((b'A' + (i as u8)) as char).to_string());
            let sm = cast::<TestBehaviour>(&*plan.behaviour).unwrap();
            assert_eq!(sm.entry_count, 1);
            assert_eq!(sm.run_count, 0);
            assert_eq!(sm.exit_count, 0);
        }
        root_plan.exit_all();
        for plan in &root_plan.plans {
            assert!(!plan.active());
            let sm = cast::<TestBehaviour>(&*plan.behaviour).unwrap();
            assert_eq!(sm.exit_count, 1);
        }
    }

    #[test]
    fn cycle_plans() {
        let _ = env_logger::try_init();
        let mut root_plan = new_plan("root", true);
        root_plan.transitions = vec![
            Transition {
                src: vec!["A".into()],
                dst: vec!["B".into()],
                predicate: Box::new(Or(vec![Box::new(True), Box::new(False)])),
            },
            Transition {
                src: vec!["B".into()],
                dst: vec!["C".into()],
                predicate: Box::new(True),
            },
            Transition {
                src: vec!["C".into()],
                dst: vec!["A".into()],
                predicate: Box::new(True),
            },
        ];
        // init plan to A
        root_plan.insert(new_plan("A", true));
        root_plan.insert(new_plan("B", false));
        root_plan.insert(new_plan("C", false));
        root_plan.run();
        root_plan.run();
        let cycles = 10;
        for _ in 0..(cycles - 1) {
            assert!(!root_plan.get("A").unwrap().active());
            assert!(!root_plan.get("B").unwrap().active());
            assert!(root_plan.get("C").unwrap().active());
            root_plan.run();
            assert!(root_plan.get("A").unwrap().active());
            assert!(!root_plan.get("B").unwrap().active());
            assert!(!root_plan.get("C").unwrap().active());
            root_plan.run();
            assert!(!root_plan.get("A").unwrap().active());
            assert!(root_plan.get("B").unwrap().active());
            assert!(!root_plan.get("C").unwrap().active());
            root_plan.run();
        }
        root_plan.exit_all();
        for plan in &root_plan.plans {
            let sm = cast::<TestBehaviour>(&*plan.behaviour).unwrap();
            assert_eq!(sm.entry_count, cycles);
            assert_eq!(sm.exit_count, cycles);
            // off by one becase inital plan didn't run
            let run_cycles = if plan.name() == "C" {
                cycles - 1
            } else {
                cycles
            };
            assert_eq!(sm.run_count, run_cycles);
        }
        debug!("{}", serde_yaml::to_string(&root_plan).unwrap());
    }
}
