use crate::*;

/// Macro to redefine `Predicate` trait in external crates for remote enum_dispatch definition.
#[macro_export]
macro_rules! predicate_trait {
    () => {
        /// An object that implements runtime predicate evaluation logic of an active plan.
        #[enum_dispatch]
        pub trait Predicate: Send {
            fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool;
        }
    };
}
predicate_trait!();

/// Default set of built-in predicates to serve as example template.
#[enum_dispatch(Predicate)]
#[derive(Serialize, Deserialize)]
pub enum Predicates {
    True,
    False,
    And(And<Self>),
    Or(Or<Self>),
    Xor(Xor<Self>),
    Not(Not<Self>),
    Nand(Nand<Self>),
    Nor(Nor<Self>),
    Xnor(Xnor<Self>),

    AllSuccess,
    AnySuccess,
    AllFailure,
    AnyFailure,
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
pub struct And<P>(pub Vec<P>);
impl<P: Predicate> Predicate for And<P> {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        self.0.iter().all(|pred| pred.evaluate(plan, src))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Or<P>(pub Vec<P>);
impl<P: Predicate> Predicate for Or<P> {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        self.0.iter().any(|pred| pred.evaluate(plan, src))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Xor<P>(pub Vec<P>);
impl<P: Predicate> Predicate for Xor<P> {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        0 != 1 & self.0.iter().filter(|x| x.evaluate(plan, src)).count()
    }
}

#[derive(Serialize, Deserialize)]
pub struct Not<P>(pub Box<P>);
impl<P: Predicate> Predicate for Not<P> {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        !self.0.evaluate(plan, src)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Nand<P>(pub Vec<P>);
impl<P: Predicate> Predicate for Nand<P> {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        !self.0.iter().all(|pred| pred.evaluate(plan, src))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Nor<P>(pub Vec<P>);
impl<P: Predicate> Predicate for Nor<P> {
    fn evaluate(&self, plan: &Plan<impl Config>, src: &[String]) -> bool {
        !self.0.iter().any(|pred| pred.evaluate(plan, src))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Xnor<P>(pub Vec<P>);
impl<P: Predicate> Predicate for Xnor<P> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize)]
    pub struct SetStatusBehaviour(pub Option<bool>);
    impl Behaviour for SetStatusBehaviour {
        fn status(&self, _: &Plan<impl Config>) -> Option<bool> {
            self.0
        }
    }

    impl From<behaviour::DefaultBehaviour> for SetStatusBehaviour {
        fn from(_: behaviour::DefaultBehaviour) -> Self {
            Self(None)
        }
    }

    #[enum_dispatch(Predicate)]
    #[derive(Serialize, Deserialize)]
    enum TestPredicate {
        True,
        False,
    }

    #[derive(Serialize, Deserialize)]
    struct TestConfig;
    impl Config for TestConfig {
        type Behaviour = SetStatusBehaviour;
        type Predicate = TestPredicate;
    }

    #[test]
    fn and() {
        let p = Plan::<TestConfig>::new(
            behaviour::DefaultBehaviour.into(),
            "",
            false,
            Duration::new(0, 0),
        );
        assert!(!And::<TestPredicate>(vec![False.into(), False.into()]).evaluate(&p, &[]));
        assert!(!And::<TestPredicate>(vec![False.into(), True.into()]).evaluate(&p, &[]));
        assert!(!And::<TestPredicate>(vec![True.into(), False.into()]).evaluate(&p, &[]));
        assert!(And::<TestPredicate>(vec![True.into(), True.into()]).evaluate(&p, &[]));
    }

    #[test]
    fn or() {
        let p = Plan::<TestConfig>::new(
            behaviour::DefaultBehaviour.into(),
            "",
            false,
            Duration::new(0, 0),
        );
        assert!(!Or::<TestPredicate>(vec![False.into(), False.into()]).evaluate(&p, &[]));
        assert!(Or::<TestPredicate>(vec![False.into(), True.into()]).evaluate(&p, &[]));
        assert!(Or::<TestPredicate>(vec![True.into(), False.into()]).evaluate(&p, &[]));
        assert!(Or::<TestPredicate>(vec![True.into(), True.into()]).evaluate(&p, &[]));
    }

    #[test]
    fn not() {
        let p = Plan::<TestConfig>::new(
            behaviour::DefaultBehaviour.into(),
            "",
            false,
            Duration::new(0, 0),
        );
        assert!(!Not::<TestPredicate>(Box::new(True.into())).evaluate(&p, &[]));
        assert!(Not::<TestPredicate>(Box::new(False.into())).evaluate(&p, &[]));
    }

    #[test]
    fn xor() {
        let p = Plan::<TestConfig>::new(
            behaviour::DefaultBehaviour.into(),
            "",
            false,
            Duration::new(0, 0),
        );
        assert!(!Xor::<TestPredicate>(vec![False.into(), False.into()]).evaluate(&p, &[]));
        assert!(Xor::<TestPredicate>(vec![False.into(), True.into()]).evaluate(&p, &[]));
        assert!(Xor::<TestPredicate>(vec![True.into(), False.into()]).evaluate(&p, &[]));
        assert!(!Xor::<TestPredicate>(vec![True.into(), True.into()]).evaluate(&p, &[]));
    }

    #[test]
    fn nand() {
        let p = Plan::<TestConfig>::new(
            behaviour::DefaultBehaviour.into(),
            "",
            false,
            Duration::new(0, 0),
        );
        assert!(Nand::<TestPredicate>(vec![False.into(), False.into()]).evaluate(&p, &[]));
        assert!(Nand::<TestPredicate>(vec![False.into(), True.into()]).evaluate(&p, &[]));
        assert!(Nand::<TestPredicate>(vec![True.into(), False.into()]).evaluate(&p, &[]));
        assert!(!Nand::<TestPredicate>(vec![True.into(), True.into()]).evaluate(&p, &[]));
    }

    #[test]
    fn nor() {
        let p = Plan::<TestConfig>::new(
            behaviour::DefaultBehaviour.into(),
            "",
            false,
            Duration::new(0, 0),
        );
        assert!(Nor::<TestPredicate>(vec![False.into(), False.into()]).evaluate(&p, &[]));
        assert!(!Nor::<TestPredicate>(vec![False.into(), True.into()]).evaluate(&p, &[]));
        assert!(!Nor::<TestPredicate>(vec![True.into(), False.into()]).evaluate(&p, &[]));
        assert!(!Nor::<TestPredicate>(vec![True.into(), True.into()]).evaluate(&p, &[]));
    }

    #[test]
    fn xnor() {
        let p = Plan::<TestConfig>::new(
            behaviour::DefaultBehaviour.into(),
            "",
            false,
            Duration::new(0, 0),
        );
        assert!(Xnor::<TestPredicate>(vec![False.into(), False.into()]).evaluate(&p, &[]));
        assert!(!Xnor::<TestPredicate>(vec![False.into(), True.into()]).evaluate(&p, &[]));
        assert!(!Xnor::<TestPredicate>(vec![True.into(), False.into()]).evaluate(&p, &[]));
        assert!(Xnor::<TestPredicate>(vec![True.into(), True.into()]).evaluate(&p, &[]));
    }

    fn make_plan(a: bool, b: bool, c: Option<bool>) -> Plan<impl Config> {
        let mut p = Plan::<TestConfig>::new(
            behaviour::DefaultBehaviour.into(),
            "",
            false,
            Duration::new(0, 0),
        );
        p.insert(Plan::<TestConfig>::new(
            SetStatusBehaviour(Some(a)),
            "a",
            false,
            Duration::new(0, 0),
        ));
        p.insert(Plan::<TestConfig>::new(
            SetStatusBehaviour(Some(b)),
            "b",
            false,
            Duration::new(0, 0),
        ));
        p.insert(Plan::<TestConfig>::new(
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
