use crate::behaviour::{Behaviour, DefaultBehaviour};
use crate::plan::{Config, Plan};

use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};

#[enum_dispatch]
#[derive(Serialize, Deserialize)]
pub enum PredicateEnum {
    True,
    False,
    And,
    Or,
    Xor,
    Not,
    Nand,
    Nor,
    Xnor,

    AllSuccess,
    AnySuccess,
    AllFailure,
    AnyFailure,
}

#[enum_dispatch(PredicateEnum)]
pub trait Predicate: Send {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool;
}

#[derive(Serialize, Deserialize)]
pub struct True;
impl Predicate for True {
    fn evaluate(&self, _: &Plan<impl Config>, _: &[String]) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize)]
pub struct False;
impl Predicate for False {
    fn evaluate(&self, _: &Plan<impl Config>, _: &[String]) -> bool {
        false
    }
}

#[derive(Serialize, Deserialize)]
pub struct And(pub Vec<PredicateEnum>);
impl Predicate for And {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        self.0.iter().all(|pred| pred.evaluate(plan, src))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Or(pub Vec<PredicateEnum>);
impl Predicate for Or {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        self.0.iter().any(|pred| pred.evaluate(plan, src))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Xor(pub Vec<PredicateEnum>);
impl Predicate for Xor {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        0 != 1 & self.0.iter().filter(|x| x.evaluate(plan, src)).count()
    }
}

#[derive(Serialize, Deserialize)]
pub struct Not(pub Box<PredicateEnum>);
impl Predicate for Not {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        !self.0.evaluate(plan, src)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Nand(pub Vec<PredicateEnum>);
impl Predicate for Nand {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        !self.0.iter().all(|pred| pred.evaluate(plan, src))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Nor(pub Vec<PredicateEnum>);
impl Predicate for Nor {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        !self.0.iter().any(|pred| pred.evaluate(plan, src))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Xnor(pub Vec<PredicateEnum>);
impl Predicate for Xnor {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        0 == 1 & self.0.iter().filter(|x| x.evaluate(plan, src)).count()
    }
}

#[derive(Serialize, Deserialize)]
pub struct AllSuccess;
impl Predicate for AllSuccess {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        all_success(plan, src, false)
    }
}

#[derive(Serialize, Deserialize)]
pub struct AnySuccess;
impl Predicate for AnySuccess {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        any_success(plan, src, false)
    }
}

#[derive(Serialize, Deserialize)]
pub struct AllFailure;
impl Predicate for AllFailure {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        !any_success(plan, src, true)
    }
}

#[derive(Serialize, Deserialize)]
pub struct AnyFailure;
impl Predicate for AnyFailure {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        !all_success(plan, src, true)
    }
}

fn all_success<C: Config>(plan: &Plan<C>, src: &[String], none_val: bool) -> bool {
    let f = |p: &Plan<C>| p.behaviour.status(&p).unwrap_or(none_val);
    if src.is_empty() {
        plan.plans.iter().all(f)
    } else {
        src.iter().filter_map(|p| plan.get(p)).all(f)
    }
}

fn any_success<C: Config>(plan: &Plan<C>, src: &[String], none_val: bool) -> bool {
    let f = |p: &Plan<C>| p.behaviour.status(&p).unwrap_or(none_val);
    if src.is_empty() {
        plan.plans.iter().any(f)
    } else {
        src.iter().filter_map(|p| plan.get(p)).any(f)
    }
}

/*
#[cfg(test)]
mod tests {
    use crate::behaviour::*;
    use crate::predicate::*;
    use std::time::Duration;

    #[test]
    fn and() {
        let p = Plan<impl Config>::new(DefaultBehaviour, "", false, Duration::new(0, 0));
        assert!(!And(vec![False.into(), False.into()]).evaluate(&p, &[]));
        assert!(!And(vec![False.into(), True.into()]).evaluate(&p, &[]));
        assert!(!And(vec![True.into(), False.into()]).evaluate(&p, &[]));
        assert!(And(vec![True.into(), True.into()]).evaluate(&p, &[]));
    }

    #[test]
    fn or() {
        let p = Plan<impl Config>::new(DefaultBehaviour, "", false, Duration::new(0, 0));
        assert!(!Or(vec![False.into(), False.into()]).evaluate(&p, &[]));
        assert!(Or(vec![False.into(), True.into()]).evaluate(&p, &[]));
        assert!(Or(vec![True.into(), False.into()]).evaluate(&p, &[]));
        assert!(Or(vec![True.into(), True.into()]).evaluate(&p, &[]));
    }

    #[test]
    fn not() {
        let p = Plan<impl Config>::new(DefaultBehaviour, "", false, Duration::new(0, 0));
        assert!(!Not(Box::new(True.into())).evaluate(&p, &[]));
        assert!(Not(Box::new(False.into())).evaluate(&p, &[]));
    }

    #[test]
    fn xor() {
        let p = Plan<impl Config>::new(DefaultBehaviour, "", false, Duration::new(0, 0));
        assert!(!Xor(vec![False.into(), False.into()]).evaluate(&p, &[]));
        assert!(Xor(vec![False.into(), True.into()]).evaluate(&p, &[]));
        assert!(Xor(vec![True.into(), False.into()]).evaluate(&p, &[]));
        assert!(!Xor(vec![True.into(), True.into()]).evaluate(&p, &[]));
    }

    #[test]
    fn nand() {
        let p = Plan<impl Config>::new(DefaultBehaviour, "", false, Duration::new(0, 0));
        assert!(Nand(vec![False.into(), False.into()]).evaluate(&p, &[]));
        assert!(Nand(vec![False.into(), True.into()]).evaluate(&p, &[]));
        assert!(Nand(vec![True.into(), False.into()]).evaluate(&p, &[]));
        assert!(!Nand(vec![True.into(), True.into()]).evaluate(&p, &[]));
    }

    #[test]
    fn nor() {
        let p = Plan<impl Config>::new(DefaultBehaviour, "", false, Duration::new(0, 0));
        assert!(Nor(vec![False.into(), False.into()]).evaluate(&p, &[]));
        assert!(!Nor(vec![False.into(), True.into()]).evaluate(&p, &[]));
        assert!(!Nor(vec![True.into(), False.into()]).evaluate(&p, &[]));
        assert!(!Nor(vec![True.into(), True.into()]).evaluate(&p, &[]));
    }

    #[test]
    fn xnor() {
        let p = Plan<impl Config>::new(DefaultBehaviour, "", false, Duration::new(0, 0));
        assert!(Xnor(vec![False.into(), False.into()]).evaluate(&p, &[]));
        assert!(!Xnor(vec![False.into(), True.into()]).evaluate(&p, &[]));
        assert!(!Xnor(vec![True.into(), False.into()]).evaluate(&p, &[]));
        assert!(Xnor(vec![True.into(), True.into()]).evaluate(&p, &[]));
    }

    fn make_plan(a: bool, b: bool, c: Option<bool>) -> Plan<impl Config> {
        let mut p = Plan<impl Config>::new(DefaultBehaviour, "", false, Duration::new(0, 0));
        p.insert(Plan<impl Config>::new(
            SetStatusBehaviour(Some(a)),
            "a",
            false,
            Duration::new(0, 0),
        ));
        p.insert(Plan<impl Config>::new(
            SetStatusBehaviour(Some(b)),
            "b",
            false,
            Duration::new(0, 0),
        ));
        p.insert(Plan<impl Config>::new(
            SetStatusBehaviour(c),
            "c",
            false,
            Duration::new(0, 0),
        ));
        p
    }

    #[test]
    fn all_success() {
        let op = AllSuccess;
        let src = Vec::<String>::from(["a".into(), "b".into(), "c".into()]);
        assert!(!op.evaluate(&make_plan(false, false, Some(false)), &src));
        assert!(!op.evaluate(&make_plan(false, true, Some(false)), &src));
        assert!(!op.evaluate(&make_plan(true, false, Some(false)), &src));
        assert!(!op.evaluate(&make_plan(true, true, Some(false)), &src));

        assert!(!op.evaluate(&make_plan(false, false, None), &src));
        assert!(!op.evaluate(&make_plan(false, true, None), &src));
        assert!(!op.evaluate(&make_plan(true, false, None), &src));
        assert!(!op.evaluate(&make_plan(true, true, None), &src));

        assert!(!op.evaluate(&make_plan(false, false, Some(true)), &src));
        assert!(!op.evaluate(&make_plan(false, true, Some(true)), &src));
        assert!(!op.evaluate(&make_plan(true, false, Some(true)), &src));
        assert!(op.evaluate(&make_plan(true, true, Some(true)), &src));
    }

    #[test]
    fn any_success() {
        let op = AnySuccess;
        let src = Vec::<String>::from(["a".into(), "b".into(), "c".into()]);
        assert!(!op.evaluate(&make_plan(false, false, Some(false)), &src));
        assert!(op.evaluate(&make_plan(false, true, Some(false)), &src));
        assert!(op.evaluate(&make_plan(true, false, Some(false)), &src));
        assert!(op.evaluate(&make_plan(true, true, Some(false)), &src));

        assert!(!op.evaluate(&make_plan(false, false, None), &src));
        assert!(op.evaluate(&make_plan(false, true, None), &src));
        assert!(op.evaluate(&make_plan(true, false, None), &src));
        assert!(op.evaluate(&make_plan(true, true, None), &src));

        assert!(op.evaluate(&make_plan(false, false, Some(true)), &src));
        assert!(op.evaluate(&make_plan(false, true, Some(true)), &src));
        assert!(op.evaluate(&make_plan(true, false, Some(true)), &src));
        assert!(op.evaluate(&make_plan(true, true, Some(true)), &src));
    }

    #[test]
    fn all_failure() {
        let op = AllFailure;
        let src = Vec::<String>::from(["a".into(), "b".into(), "c".into()]);
        assert!(op.evaluate(&make_plan(false, false, Some(false)), &src));
        assert!(!op.evaluate(&make_plan(false, true, Some(false)), &src));
        assert!(!op.evaluate(&make_plan(true, false, Some(false)), &src));
        assert!(!op.evaluate(&make_plan(true, true, Some(false)), &src));

        assert!(!op.evaluate(&make_plan(false, false, None), &src));
        assert!(!op.evaluate(&make_plan(false, true, None), &src));
        assert!(!op.evaluate(&make_plan(true, false, None), &src));
        assert!(!op.evaluate(&make_plan(true, true, None), &src));

        assert!(!op.evaluate(&make_plan(false, false, Some(true)), &src));
        assert!(!op.evaluate(&make_plan(false, true, Some(true)), &src));
        assert!(!op.evaluate(&make_plan(true, false, Some(true)), &src));
        assert!(!op.evaluate(&make_plan(true, true, Some(true)), &src));
    }

    #[test]
    fn any_failure() {
        let op = AnyFailure;
        let src = Vec::<String>::from(["a".into(), "b".into(), "c".into()]);
        assert!(op.evaluate(&make_plan(false, false, Some(false)), &src));
        assert!(op.evaluate(&make_plan(false, true, Some(false)), &src));
        assert!(op.evaluate(&make_plan(true, false, Some(false)), &src));
        assert!(op.evaluate(&make_plan(true, true, Some(false)), &src));

        assert!(op.evaluate(&make_plan(false, false, None), &src));
        assert!(op.evaluate(&make_plan(false, true, None), &src));
        assert!(op.evaluate(&make_plan(true, false, None), &src));
        assert!(!op.evaluate(&make_plan(true, true, None), &src));

        assert!(op.evaluate(&make_plan(false, false, Some(true)), &src));
        assert!(op.evaluate(&make_plan(false, true, Some(true)), &src));
        assert!(op.evaluate(&make_plan(true, false, Some(true)), &src));
        assert!(!op.evaluate(&make_plan(true, true, Some(true)), &src));
    }
}
*/
