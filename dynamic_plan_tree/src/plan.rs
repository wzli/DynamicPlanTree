use crate::*;

#[cfg(feature = "rayon")]
use rayon::prelude::*;

#[cfg(feature = "serde")]
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use tracing::{debug, debug_span, Span};

/// A user provided object to statically pass in custom implementation for `Behaviour` and `Predicate`.
pub trait Config: Sized + 'static {
    #[cfg(all(feature = "rayon", feature = "serde"))]
    type Predicate: Predicate + Send + Serialize + DeserializeOwned + EnumCast;
    #[cfg(all(not(feature = "rayon"), feature = "serde"))]
    type Predicate: Predicate + Serialize + DeserializeOwned + EnumCast;
    #[cfg(all(feature = "rayon", not(feature = "serde")))]
    type Predicate: Predicate + Send + EnumCast;
    #[cfg(all(not(feature = "rayon"), not(feature = "serde")))]
    type Predicate: Predicate + EnumCast;

    #[cfg(all(feature = "rayon", feature = "serde"))]
    type Behaviour: Behaviour<Self> + Send + Serialize + DeserializeOwned + EnumCast;
    #[cfg(all(not(feature = "rayon"), feature = "serde"))]
    type Behaviour: Behaviour<Self> + Serialize + DeserializeOwned + EnumCast;
    #[cfg(all(feature = "rayon", not(feature = "serde")))]
    type Behaviour: Behaviour<Self> + Send + EnumCast;
    #[cfg(all(not(feature = "rayon"), not(feature = "serde")))]
    type Behaviour: Behaviour<Self> + EnumCast;
}

/// Transition from `src` plans to `dst` plans within the parent plan upon the result of `predicate` evaluation.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Transition<P> {
    pub src: Vec<String>,
    pub dst: Vec<String>,
    pub predicate: P,
}

/// A node in the plan tree containing some behaviour, subplans, and possible transitions.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Plan<C: Config> {
    name: String,
    active: bool,
    run_countdown: u32,
    /// Number of ticks between each run.
    pub run_interval: u32,
    /// Automatically enter follwing the entry of parent plan.
    pub autostart: bool,
    /// Customizable runtime logic.
    pub behaviour: Option<Box<C::Behaviour>>,
    /// List of transition conditions between sets of subplans.
    pub transitions: Vec<Transition<C::Predicate>>,
    /// Contains instances of subplans recursively.
    pub plans: Vec<Self>,
    /// Storage for arbituary serializable data.
    pub data: HashMap<String, serde_value::Value>,
    #[cfg_attr(feature = "serde", serde(skip, default = "Span::none"))]
    span: Span,
}

impl<C: Config> Plan<C> {
    /// ID unique amongst subplans.
    pub fn name(&self) -> &String {
        &self.name
    }

    /// Whether the inner behaviour is scheduled to run.
    pub fn active(&self) -> bool {
        self.active
    }

    /// Number of ticks until next run.
    pub fn run_countdown(&self) -> u32 {
        self.run_countdown
    }

    /// Status of the inner behaviour.
    pub fn status(&self) -> Option<bool> {
        self.behaviour.as_ref()?.status(self)
    }

    /// Utility of the inner behaviour.
    pub fn utility(&self) -> f64 {
        self.behaviour
            .as_ref()
            .map(|b| b.utility(self))
            .unwrap_or(0.)
    }

    /// New plan with behaviour and no subplans.
    pub fn new(
        behaviour: C::Behaviour,
        name: impl Into<String>,
        run_interval: u32,
        autostart: bool,
    ) -> Self {
        let mut s = Self::new_stub(name, autostart);
        s.run_interval = run_interval;
        s.behaviour = Some(Box::new(behaviour));
        s
    }

    /// New plan without any behaviour.
    pub fn new_stub(name: impl Into<String>, autostart: bool) -> Self {
        Self {
            name: name.into(),
            active: false,
            run_countdown: 0,
            run_interval: 0,
            autostart,
            behaviour: None,
            transitions: Vec::new(),
            plans: Vec::new(),
            data: HashMap::new(),
            span: Span::none(),
        }
    }

    pub fn insert(&mut self, mut plan: Self) -> &mut Self {
        debug!(parent: &self.span, plan=%plan.name, "insert");
        if plan.active {
            if self.active {
                // create new span if this plan and inserted plan is active
                plan.span = debug_span!(parent: &self.span, "plan", name=%plan.name);
            } else {
                // exit inserted span if this plan is inactive
                plan.exit(false);
            }
        }
        // sorted insert
        let (pos, _) = match self.find(&plan.name) {
            // overwrite if there is already one
            Ok(pos) => (pos, self.plans[pos] = plan),
            Err(pos) => (pos, self.plans.insert(pos, plan)),
        };
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

    pub fn cast<B: Behaviour<C>>(&self) -> Option<&B> {
        self.behaviour.as_ref()?.cast::<B>()
    }

    pub fn cast_mut<B: Behaviour<C>>(&mut self) -> Option<&mut B> {
        self.behaviour.as_mut()?.cast_mut::<B>()
    }

    pub fn get_cast<B: Behaviour<C>>(&self, name: &str) -> Option<&B> {
        self.get(name)?.cast::<B>()
    }

    pub fn get_cast_mut<B: Behaviour<C>>(&mut self, name: &str) -> Option<&mut B> {
        self.get_mut(name)?.cast_mut::<B>()
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
        debug!(parent: &self.span, plan=?self.name(), active=?active_plans);

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

        // call on_prepare() before children behaviours run()
        if self.run_interval > 0 && self.run_countdown == 0 {
            self.call(|behaviour, plan| behaviour.on_prepare(plan), "prepare");
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
                self.plans.insert(pos, Self::new_stub(name, false));
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
        // trigger on_entry() for self
        self.active = true;
        self.call(|behaviour, plan| behaviour.on_entry(plan), "entry");
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

    fn call(&mut self, f: impl FnOnce(&mut Box<C::Behaviour>, &mut Self), name: &str) {
        let mut behaviour = std::mem::take(&mut self.behaviour);
        if let Some(b) = &mut behaviour {
            let _span = debug_span!(parent: &self.span, "call", func=%name).entered();
            f(b, self);
            self.behaviour = behaviour;
        }
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

    #[derive(Default, Debug, EnumCast)]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct RunCountBehaviour {
        pub entry_count: u32,
        pub exit_count: u32,
        pub run_count: u32,
    }

    impl<C: Config> Behaviour<C> for RunCountBehaviour {
        fn status(&self, _plan: &Plan<C>) -> Option<bool> {
            None
        }
        fn on_entry(&mut self, plan: &mut Plan<C>) {
            self.entry_count += 1;
            assert!(plan.behaviour.is_none())
        }
        fn on_exit(&mut self, _plan: &mut Plan<C>) {
            self.exit_count += 1;
        }
        fn on_run(&mut self, _plan: &mut Plan<C>) {
            self.run_count += 1;
        }
    }

    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    struct TestConfig;
    impl Config for TestConfig {
        type Predicate = predicate::Predicates;
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
                predicate: predicate::True.into_enum().unwrap(),
            },
            Transition {
                src: vec!["B".into()],
                dst: vec!["C".into()],
                predicate: predicate::True.into_enum().unwrap(),
            },
            Transition {
                src: vec!["C".into()],
                dst: vec!["A".into()],
                predicate: predicate::True.into_enum().unwrap(),
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
            let sm = plan.behaviour.as_ref().unwrap();
            assert_eq!(sm.entry_count, 0);
            assert_eq!(sm.run_count, 0);
            assert_eq!(sm.exit_count, 0);
        }
        root_plan.exit(false);
        for plan in &root_plan.plans {
            assert!(!plan.active());
            assert_eq!(plan.behaviour.as_ref().unwrap().exit_count, 0);
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
            let sm = plan.behaviour.as_ref().unwrap();
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

    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    struct DefaultConfig;
    impl Config for DefaultConfig {
        type Predicate = predicate::Predicates;
        type Behaviour = behaviour::Behaviours<Self>;
    }

    #[test]
    #[cfg(feature = "serde")]
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
    #[cfg(feature = "serde")]
    fn generate_plan() {
        tracing_init();
        let root_plan = Plan::<DefaultConfig>::new_stub("root", true);
        // serialize and print root plan
        debug!("{}", serde_json::to_string_pretty(&root_plan).unwrap());
    }

    #[test]
    fn downcast() {
        use behaviour::*;
        type B = Behaviours<DefaultConfig>;
        let mut a: B = AnySuccessStatus.into();
        let mut b: B = AllSuccessStatus.into();
        a.cast::<AnySuccessStatus>().unwrap();
        b.cast::<AllSuccessStatus>().unwrap();
        a.cast_mut::<AnySuccessStatus>().unwrap();
        b.cast_mut::<AllSuccessStatus>().unwrap();
    }
}
