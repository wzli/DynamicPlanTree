use crate::*;

#[cfg(feature = "rayon")]
use rayon::prelude::*;

use serde::de::DeserializeOwned;
use std::collections::HashMap;
use tracing::{debug, debug_span, Span};

pub use serde_value::Value;

/// A user provided object to statically pass in custom implementation for `Behaviour` and `Predicate`.
pub trait Config: Sized + 'static {
    #[cfg(feature = "rayon")]
    type Predicate: Predicate + Send + Serialize + DeserializeOwned + FromAny;
    #[cfg(not(feature = "rayon"))]
    type Predicate: Predicate + Serialize + DeserializeOwned + FromAny;

    #[cfg(feature = "rayon")]
    type Behaviour: Behaviour<Self> + Send + Serialize + DeserializeOwned + FromAny;
    #[cfg(not(feature = "rayon"))]
    type Behaviour: Behaviour<Self> + Serialize + DeserializeOwned + FromAny;
}

pub trait FromAny: Sized {
    fn from_any(x: impl Any) -> Option<Self>;
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
    run_countdown: u32,
    pub run_interval: u32,
    pub autostart: bool,
    pub behaviour: Box<C::Behaviour>,
    pub transitions: Vec<Transition<C::Predicate>>,
    pub plans: Vec<Self>,
    pub data: HashMap<String, Value>,
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

    pub fn run_countdown(&self) -> u32 {
        self.run_countdown
    }

    pub fn status(&self) -> Option<bool> {
        self.behaviour.status(self)
    }

    pub fn utility(&self) -> f64 {
        self.behaviour.utility(self)
    }

    pub fn new(
        behaviour: C::Behaviour,
        name: impl Into<String>,
        run_interval: u32,
        autostart: bool,
    ) -> Self {
        Self {
            name: name.into(),
            active: false,
            run_countdown: 0,
            run_interval,
            autostart,
            behaviour: Box::new(behaviour),
            transitions: Vec::new(),
            plans: Vec::new(),
            data: HashMap::new(),
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

        // call on_pre_run() before children behaviours run()
        if self.run_interval > 0 && self.run_countdown == 0 {
            self.call(|behaviour, plan| behaviour.on_pre_run(plan), "pre_run");
        }

        // call run() recursively
        let i = self.plans.iter_mut().filter(|plan| plan.active);
        #[cfg(feature = "rayon")]
        i.par_bridge().for_each(|plan| plan.run());
        #[cfg(not(feature = "rayon"))]
        i.for_each(|plan| plan.run());

        // limit execution frequency
        if self.run_interval == 0 {
            return;
        }
        if self.run_countdown == 0 {
            // run the behaviour of this plan
            self.call(|behaviour, plan| behaviour.on_run(plan), "run");
            self.run_countdown = self.run_interval;
        }
        self.run_countdown -= 1;
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
                        C::Behaviour::from_any(behaviour::DefaultBehaviour).unwrap(),
                        name,
                        0,
                        false,
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
        // trigger on_entry() for self
        self.active = true;
        self.call(|behaviour, plan| behaviour.on_entry(plan), "entry");
        // create new span
        match parent_span {
            Some(x) => self.span = debug_span!(parent: x, "plan", name=%self.name),
            None => self.span = debug_span!("plan", name=%self.name),
        }
        // recursively enter all autostart child plans
        let i = self
            .plans
            .iter_mut()
            .filter(|plan| plan.autostart && !plan.active);
        #[cfg(feature = "rayon")]
        i.par_bridge().for_each(|plan| {
            plan.enter(Some(&self.span));
        });
        #[cfg(not(feature = "rayon"))]
        i.for_each(|plan| {
            plan.enter(Some(&self.span));
        });
        true
    }

    pub fn exit(&mut self, exclude_self: bool) -> bool {
        // only exit if plan is active
        if !self.active {
            return false;
        }
        // recursively exit all active child plans
        let i = self.plans.iter_mut().filter(|plan| plan.active);
        #[cfg(feature = "rayon")]
        i.par_bridge().for_each(|plan| {
            plan.exit(false);
        });
        #[cfg(not(feature = "rayon"))]
        i.for_each(|plan| {
            plan.exit(false);
        });
        // trigger on_exit() for self
        if !exclude_self {
            self.call(|behaviour, plan| behaviour.on_exit(plan), "exit");
            self.active = false;
            self.span = Span::none();
        }
        true
    }

    fn call<T>(&mut self, f: impl FnOnce(&mut Box<C::Behaviour>, &mut Self) -> T, name: &str) -> T {
        let _span = debug_span!(parent: &self.span, "call", func=%name).entered();
        let default = Box::new(C::Behaviour::from_any(behaviour::DefaultBehaviour).unwrap());
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
            _plan
                .behaviour
                .as_any()
                .downcast_ref::<RunCountBehaviour>()
                .unwrap();
        }
        fn on_exit(&mut self, _plan: &mut Plan<C>) {
            self.exit_count += 1;
        }
        fn on_run(&mut self, _plan: &mut Plan<C>) {
            self.run_count += 1;
        }
    }

    impl FromAny for RunCountBehaviour {
        fn from_any(_: impl Any) -> Option<Self> {
            Some(Self::default())
        }
    }

    #[derive(Serialize, Deserialize)]
    struct TestConfig;
    impl Config for TestConfig {
        type Predicate = predicate::True;
        type Behaviour = RunCountBehaviour;
    }

    fn new_plan(name: &str, autostart: bool) -> Plan<TestConfig> {
        Plan::<TestConfig>::new(RunCountBehaviour::default(), name, 1, autostart)
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

    #[derive(Serialize, Deserialize)]
    struct DefaultConfig;
    impl Config for DefaultConfig {
        type Predicate = predicate::Predicates;
        type Behaviour = behaviour::Behaviours<Self>;
    }

    #[test]
    fn generate_schema() {
        tracing_init();
        // generate and print plan schema
        use serde_reflection::{Tracer, TracerConfig};
        let mut tracer = Tracer::new(TracerConfig::default());
        tracer
            .trace_simple_type::<behaviour::Behaviours<DefaultConfig>>()
            .unwrap();
        tracer.trace_simple_type::<predicate::Predicates>().unwrap();
        let registry = tracer.registry().unwrap();
        debug!("{}", serde_json::to_string_pretty(&registry).unwrap());
    }

    #[test]
    fn generate_plan() {
        tracing_init();
        let root_plan =
            Plan::<DefaultConfig>::new(behaviour::DefaultBehaviour.into(), "root", 1, true);
        // serialize and print root plan
        debug!("{}", serde_json::to_string_pretty(&root_plan).unwrap());
    }

    #[test]
    fn downcast() {
        use behaviour::*;
        type B = Behaviours<DefaultConfig>;
        let mut a: B = DefaultBehaviour.into();
        let mut b: B = AllSuccessStatus.into();
        a.as_any().downcast_ref::<DefaultBehaviour>().unwrap();
        b.as_any().downcast_ref::<AllSuccessStatus>().unwrap();
        a.as_any_mut().downcast_mut::<DefaultBehaviour>().unwrap();
        b.as_any_mut().downcast_mut::<AllSuccessStatus>().unwrap();
    }
}
