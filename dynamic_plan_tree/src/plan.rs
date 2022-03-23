use crate::behaviour::{Behaviour, BehaviourEnum, DefaultBehaviour};
use crate::predicate::{Predicate, PredicateEnum};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, Instant};
use tracing::{debug, debug_span, Span};

#[derive(Serialize, Deserialize)]
pub struct Transition {
    pub src: Vec<String>,
    pub dst: Vec<String>,
    pub predicate: PredicateEnum,
}

#[derive(Serialize, Deserialize)]
pub struct Plan {
    name: String,
    active: bool,
    pub autostart: bool,
    #[serde(with = "serde_millis")]
    pub run_interval: Duration,
    pub behaviour: Box<BehaviourEnum>,
    pub transitions: Vec<Transition>,
    pub plans: Vec<Plan>,
    pub data: Value,
    #[serde(skip, default = "Instant::now")]
    timestamp: Instant,
    #[serde(skip, default = "Span::none")]
    span: Span,
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

    pub fn new(
        behaviour: impl Into<BehaviourEnum>,
        name: impl Into<String>,
        autostart: bool,
        run_interval: Duration,
    ) -> Self {
        Self {
            name: name.into(),
            active: false,
            autostart,
            run_interval,
            behaviour: Box::new(behaviour.into()),
            transitions: Vec::new(),
            plans: Vec::new(),
            data: Value::Null,
            timestamp: Instant::now(),
            span: Span::none(),
        }
    }

    pub fn insert(&mut self, mut plan: Plan) -> &mut Plan {
        // sorted insert
        let found = self.find(&plan.name);
        let pos = found.unwrap_or_else(|x| x);
        debug!(parent: &self.span, plan=%plan.name, "insert");
        if plan.active {
            if self.active {
                plan.span = debug_span!(parent: &self.span, "plan", name=%plan.name);
            } else {
                plan.exit_all();
            }
        }
        match found {
            // overwrite if there is already one
            Ok(_) => self.plans[pos] = plan,
            Err(_) => self.plans.insert(pos, plan),
        }
        &mut self.plans[pos]
    }

    pub fn remove(&mut self, name: &str) -> Option<Plan> {
        let pos = self.find(name).ok()?;
        debug!(parent: &self.span, plan=%name, "remove");
        Some(self.plans.remove(pos))
    }

    pub fn find(&self, name: &str) -> Result<usize, usize> {
        self.plans.binary_search_by(|plan| (*plan.name).cmp(name))
    }

    pub fn get(&self, name: &str) -> Option<&Plan> {
        let pos = self.find(name).ok()?;
        Some(&self.plans[pos])
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Plan> {
        let pos = self.find(name).ok()?;
        Some(&mut self.plans[pos])
    }

    pub fn run(&mut self) {
        // enter plan if not already
        self.enter_autostart(None);

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
                    self.exit(p);
                });
                t.dst.iter().filter(|p| !t.src.contains(p)).for_each(|p| {
                    self.enter(p);
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

    pub fn enter(&mut self, name: &str) -> Option<&mut Plan> {
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
                    Plan::new(DefaultBehaviour, name, false, Duration::new(0, 0)),
                );
                pos
            }
        };
        let plan = &mut self.plans[pos];
        plan.enter_autostart(Some(&self.span));
        Some(plan)
    }

    pub fn exit(&mut self, name: &str) -> Option<&mut Plan> {
        // ignore if plan is not found
        let pos = self.find(name).ok()?;
        let plan = &mut self.plans[pos];
        plan.exit_all();
        Some(plan)
    }

    pub fn enter_autostart(&mut self, parent_span: Option<&Span>) -> bool {
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
                plan.enter_autostart(Some(&self.span));
            });
        // trigger on_entry() for self
        self.active = true;
        self.call(|behaviour, plan| behaviour.on_entry(plan), "entry");
        true
    }

    pub fn exit_all(&mut self) -> bool {
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
                plan.exit_all();
            });
        // trigger on_exit() for self
        self.call(|behaviour, plan| behaviour.on_exit(plan), "exit");
        self.active = false;
        self.span = Span::none();
        true
    }

    fn call<T>(
        &mut self,
        f: impl FnOnce(&mut Box<BehaviourEnum>, &mut Plan) -> T,
        name: &str,
    ) -> T {
        let _span = debug_span!(parent: &self.span, "call", func=%name).entered();
        let mut behaviour =
            std::mem::replace(&mut self.behaviour, Box::new(DefaultBehaviour.into()));
        let ret = f(&mut behaviour, self);
        let _ = std::mem::replace(&mut self.behaviour, behaviour);
        ret
    }
}

impl Drop for Plan {
    fn drop(&mut self) {
        if self.active {
            self.call(|behaviour, plan| behaviour.on_exit(plan), "exit");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::behaviour::RunCountBehaviour;
    use crate::plan::*;
    use crate::predicate::*;

    fn new_plan(name: &str, autostart: bool) -> Plan {
        Plan::new(
            RunCountBehaviour::default(),
            name,
            autostart,
            Duration::new(0, 0),
        )
    }

    fn abc_plan() -> Plan {
        let mut root_plan = new_plan("root", true);
        root_plan.transitions = vec![
            Transition {
                src: vec!["A".into()],
                dst: vec!["B".into()],
                predicate: Or(vec![True.into(), False.into()]).into(),
            },
            Transition {
                src: vec!["B".into()],
                dst: vec!["C".into()],
                predicate: True.into(),
            },
            Transition {
                src: vec!["C".into()],
                dst: vec!["A".into()],
                predicate: True.into(),
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
        let _ = tracing_subscriber::fmt::try_init();

        let mut root_plan = new_plan("root", true);
        root_plan.insert(new_plan("C", true));
        root_plan.insert(new_plan("A", true));
        root_plan.insert(new_plan("B", true));
        root_plan.insert(new_plan("B", true));

        assert_eq!(root_plan.plans.len(), 3);
        for (i, plan) in root_plan.plans.iter().enumerate() {
            assert!(!plan.active());
            assert_eq!(plan.name(), &((b'A' + (i as u8)) as char).to_string());
            match &*plan.behaviour {
                BehaviourEnum::RunCountBehaviour(sm) => {
                    assert_eq!(sm.entry_count, 0);
                    assert_eq!(sm.run_count, 0);
                    assert_eq!(sm.exit_count, 0);
                }
                _ => panic!(),
            }
        }
        root_plan.exit_all();
        for plan in &root_plan.plans {
            assert!(!plan.active());
            match &*plan.behaviour {
                BehaviourEnum::RunCountBehaviour(sm) => {
                    assert_eq!(sm.exit_count, 0);
                }
                _ => panic!(),
            }
        }
    }

    #[test]
    fn cycle_plans() {
        use tracing::Level;
        use tracing_subscriber::fmt::format::FmtSpan;
        let _ = tracing_subscriber::fmt()
            .with_span_events(FmtSpan::ENTER)
            .with_max_level(Level::DEBUG)
            .with_target(false)
            .try_init();
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
        root_plan.exit_all();

        for plan in &root_plan.plans {
            match &*plan.behaviour {
                BehaviourEnum::RunCountBehaviour(sm) => {
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
                _ => panic!(),
            }
        }
    }

    #[test]
    fn generate_schema() {
        let _ = tracing_subscriber::fmt::try_init();

        let root_plan = abc_plan();

        // serialize and print root plan
        debug!("{}", serde_json::to_string_pretty(&root_plan).unwrap());

        // generate and print plan schema
        use serde_reflection::{Tracer, TracerConfig};
        let mut tracer = Tracer::new(TracerConfig::default());
        tracer.trace_simple_type::<BehaviourEnum>().unwrap();
        tracer.trace_simple_type::<PredicateEnum>().unwrap();
        let registry = tracer.registry().unwrap();
        debug!("{}", serde_json::to_string_pretty(&registry).unwrap());
    }
}
