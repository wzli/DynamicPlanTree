use crate::*;

use rayon::prelude::*;
use serde::de::DeserializeOwned;
use std::time::Instant;
use tracing::{debug, debug_span, Span};

pub use serde_json::Value;
pub use std::time::Duration;

/// A user provided object to statically pass in custom implementation for `Behaviour` and `Predicate`.
pub trait Config: Sized {
    type Predicate: Predicate + Serialize + DeserializeOwned;
    type Behaviour: Behaviour<Self>
        + From<behaviour::DefaultBehaviour>
        + Serialize
        + DeserializeOwned;
}

/// Transition from `src` plans to `dst` plans within the parent plan upon the result of `predicate` evaluation.
#[derive(Serialize, Deserialize)]
pub struct Transition<P> {
    pub src: Vec<String>,
    pub dst: Vec<String>,
    pub predicate: P,
}

/// A node in the plan tree containing some behaviour, children plans, and possible transitions.
#[derive(Serialize, Deserialize)]
pub struct Plan<C: Config> {
    name: String,
    active: bool,
    pub autostart: bool,
    #[serde(with = "serde_millis")]
    pub run_interval: Duration,
    pub behaviour: Box<C::Behaviour>,
    pub transitions: Vec<Transition<C::Predicate>>,
    pub plans: Vec<Self>,
    pub data: Value,
    #[serde(skip, default = "Instant::now")]
    timestamp: Instant,
    #[serde(skip, default = "Span::none")]
    span: Span,
}

impl<C: Config> Plan<C> {
    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn timestamp(&self) -> Instant {
        self.timestamp
    }

    pub fn new(
        behaviour: C::Behaviour,
        name: impl Into<String>,
        autostart: bool,
        run_interval: Duration,
    ) -> Self {
        Self {
            name: name.into(),
            active: false,
            autostart,
            run_interval,
            behaviour: Box::new(behaviour),
            transitions: Vec::new(),
            plans: Vec::new(),
            data: Value::Null,
            timestamp: Instant::now(),
            span: Span::none(),
        }
    }

    pub fn insert(&mut self, mut plan: Self) -> &mut Self {
        // sorted insert
        let found = self.find(&plan.name);
        let pos = found.unwrap_or_else(|x| x);
        debug!(parent: &self.span, plan=%plan.name, "insert");
        if plan.active {
            if self.active {
                plan.span = debug_span!(parent: &self.span, "plan", name=%plan.name);
            } else {
                plan.exit(false);
            }
        }
        match found {
            // overwrite if there is already one
            Ok(_) => self.plans[pos] = plan,
            Err(_) => self.plans.insert(pos, plan),
        }
        &mut self.plans[pos]
    }

    pub fn remove(&mut self, name: &str) -> Option<Self> {
        let pos = self.find(name).ok()?;
        debug!(parent: &self.span, plan=%name, "remove");
        Some(self.plans.remove(pos))
    }

    pub fn find(&self, name: &str) -> Result<usize, usize> {
        self.plans.binary_search_by(|plan| (*plan.name).cmp(name))
    }

    pub fn get(&self, name: &str) -> Option<&Self> {
        let pos = self.find(name).ok()?;
        Some(&self.plans[pos])
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Self> {
        let pos = self.find(name).ok()?;
        Some(&mut self.plans[pos])
    }

    pub fn run(&mut self) {
        // enter plan if not already
        self.enter(None);

        // get active set of plans
        use std::collections::HashSet;
        let active_plans = self
            .plans
            .iter()
            .filter(|plan| plan.active)
            .map(|plan| &plan.name)
            .collect::<HashSet<_>>();
        debug!(parent: &self.span, plans=?active_plans, "active");

        // evaluate state transitions
        let transitions = std::mem::take(&mut self.transitions);
        transitions
            .iter()
            .filter(|t| {
                t.src.iter().all(|plan| active_plans.contains(plan))
                    && t.predicate.evaluate(self, &t.src)
            })
            .collect::<Vec<_>>()
            .iter()
            .for_each(|t| {
                debug!(parent: &self.span, src=?t.src, dst=?t.dst, "transition");
                t.src.iter().filter(|p| !t.dst.contains(p)).for_each(|p| {
                    self.exit_plan(p);
                });
                t.dst.iter().filter(|p| !t.src.contains(p)).for_each(|p| {
                    self.enter_plan(p);
                });
            });
        let _ = std::mem::replace(&mut self.transitions, transitions);

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

        // run the state machine of this plan
        self.call(|behaviour, plan| behaviour.on_run(plan), "run");
    }

    pub fn enter_plan(&mut self, name: &str) -> Option<&mut Self> {
        // can only enter plans within an active plan
        if !self.active {
            return None;
        }
        // look for requested plan
        let pos = match self.find(name) {
            Ok(pos) => pos,
            // if plan doesn't exist, create and insert a default plan
            Err(pos) => {
                self.plans.insert(
                    pos,
                    Self::new(
                        behaviour::DefaultBehaviour.into(),
                        name,
                        false,
                        Duration::new(0, 0),
                    ),
                );
                pos
            }
        };
        let plan = &mut self.plans[pos];
        plan.enter(Some(&self.span));
        Some(plan)
    }

    pub fn exit_plan(&mut self, name: &str) -> Option<&mut Self> {
        // ignore if plan is not found
        let pos = self.find(name).ok()?;
        let plan = &mut self.plans[pos];
        plan.exit(false);
        Some(plan)
    }

    pub fn enter(&mut self, parent_span: Option<&Span>) -> bool {
        // only enter if plan is inactive
        if self.active {
            return false;
        }
        // create new span
        match parent_span {
            Some(x) => self.span = debug_span!(parent: x, "plan", name=%self.name),
            None => self.span = debug_span!("plan", name=%self.name),
        }
        // recursively enter all autostart child plans
        self.plans
            .iter_mut()
            .filter(|plan| plan.autostart && !plan.active)
            .par_bridge()
            .for_each(|plan| {
                plan.enter(Some(&self.span));
            });
        // trigger on_entry() for self
        self.active = true;
        self.call(|behaviour, plan| behaviour.on_entry(plan), "entry");
        true
    }

    pub fn exit(&mut self, exclude_self: bool) -> bool {
        // only exit if plan is active
        if !self.active {
            return false;
        }
        // recursively exit all active child plans
        self.plans
            .iter_mut()
            .filter(|plan| plan.active)
            .par_bridge()
            .for_each(|plan| {
                plan.exit(false);
            });
        if !exclude_self {
            // trigger on_exit() for self
            self.call(|behaviour, plan| behaviour.on_exit(plan), "exit");
            self.active = false;
            self.span = Span::none();
        }
        true
    }

    fn call<T>(&mut self, f: impl FnOnce(&mut Box<C::Behaviour>, &mut Self) -> T, name: &str) -> T {
        let _span = debug_span!(parent: &self.span, "call", func=%name).entered();
        let default = Box::new(behaviour::DefaultBehaviour.into());
        let mut behaviour = std::mem::replace(&mut self.behaviour, default);
        let ret = f(&mut behaviour, self);
        let _ = std::mem::replace(&mut self.behaviour, behaviour);
        ret
    }
}

impl<C: Config> Drop for Plan<C> {
    fn drop(&mut self) {
        if self.active {
            self.call(|behaviour, plan| behaviour.on_exit(plan), "exit");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracing_init() {
        use tracing_subscriber::fmt::format::FmtSpan;
        let _ = tracing_subscriber::fmt()
            .with_span_events(FmtSpan::ENTER)
            .with_target(false)
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();
    }

    #[derive(Serialize, Deserialize, Default, Debug)]
    pub struct RunCountBehaviour {
        pub entry_count: u32,
        pub exit_count: u32,
        pub run_count: u32,
    }

    impl<C: Config> Behaviour<C> for RunCountBehaviour {
        fn status(&self, _plan: &Plan<C>) -> Option<bool> {
            None
        }
        fn on_entry(&mut self, _plan: &mut Plan<C>) {
            self.entry_count += 1;
        }
        fn on_exit(&mut self, _plan: &mut Plan<C>) {
            self.exit_count += 1;
        }
        fn on_run(&mut self, _plan: &mut Plan<C>) {
            self.run_count += 1;
        }
    }

    impl From<behaviour::DefaultBehaviour> for RunCountBehaviour {
        fn from(_: behaviour::DefaultBehaviour) -> RunCountBehaviour {
            RunCountBehaviour::default()
        }
    }

    #[derive(Serialize, Deserialize)]
    struct TestConfig;
    impl Config for TestConfig {
        type Predicate = predicate::True;
        type Behaviour = RunCountBehaviour;
    }

    fn new_plan(name: &str, autostart: bool) -> Plan<TestConfig> {
        Plan::<TestConfig>::new(
            RunCountBehaviour::default(),
            name,
            autostart,
            Duration::new(0, 0),
        )
    }

    fn abc_plan() -> Plan<TestConfig> {
        let mut root_plan = new_plan("root", true);
        root_plan.transitions = vec![
            Transition {
                src: vec!["A".into()],
                dst: vec!["B".into()],
                predicate: predicate::True,
            },
            Transition {
                src: vec!["B".into()],
                dst: vec!["C".into()],
                predicate: predicate::True,
            },
            Transition {
                src: vec!["C".into()],
                dst: vec!["A".into()],
                predicate: predicate::True,
            },
        ];
        // init plan to A
        root_plan.insert(new_plan("A", true));
        root_plan.insert(new_plan("B", false));
        root_plan.insert(new_plan("C", false));
        root_plan
    }

    #[test]
    fn sorted_insert() {
        tracing_init();

        let mut root_plan = new_plan("root", true);
        root_plan.insert(new_plan("C", true));
        root_plan.insert(new_plan("A", true));
        root_plan.insert(new_plan("B", true));
        root_plan.insert(new_plan("B", true));

        assert_eq!(root_plan.plans.len(), 3);
        for (i, plan) in root_plan.plans.iter().enumerate() {
            assert!(!plan.active());
            assert_eq!(plan.name(), &((b'A' + (i as u8)) as char).to_string());
            let sm = &plan.behaviour;
            assert_eq!(sm.entry_count, 0);
            assert_eq!(sm.run_count, 0);
            assert_eq!(sm.exit_count, 0);
        }
        root_plan.exit(false);
        for plan in &root_plan.plans {
            assert!(!plan.active());
            assert_eq!(plan.behaviour.exit_count, 0);
        }
    }

    #[test]
    fn cycle_plans() {
        tracing_init();
        let mut root_plan = abc_plan();
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
        root_plan.exit(false);

        for plan in &root_plan.plans {
            let sm = &plan.behaviour;
            assert_eq!(sm.entry_count, cycles);
            assert_eq!(sm.exit_count, cycles);
            // off by one becase inital plan didn't run
            let run_cycles = if plan.name() == "A" {
                cycles - 1
            } else {
                cycles
            };
            assert_eq!(sm.run_count, run_cycles);
        }
    }
}
