use crate::*;

#[typetag::serde(tag = "type")]
pub trait Predicate: Send + Downcast {
    fn evaluate(&self, plan: &Plan, src: &HashSet<String>) -> bool;
}
impl_downcast!(Predicate);

#[derive(Serialize, Deserialize)]
pub struct True;
#[typetag::serde]
impl Predicate for True {
    fn evaluate(&self, _: &Plan, _: &HashSet<String>) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize)]
pub struct False;
#[typetag::serde]
impl Predicate for False {
    fn evaluate(&self, _: &Plan, _: &HashSet<String>) -> bool {
        false
    }
}

#[derive(Serialize, Deserialize)]
pub struct And(pub Vec<Box<dyn Predicate>>);
#[typetag::serde]
impl Predicate for And {
    fn evaluate(&self, plan: &Plan, src: &HashSet<String>) -> bool {
        self.0.iter().all(|pred| pred.evaluate(plan, src))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Or(pub Vec<Box<dyn Predicate>>);
#[typetag::serde]
impl Predicate for Or {
    fn evaluate(&self, plan: &Plan, src: &HashSet<String>) -> bool {
        self.0.iter().any(|pred| pred.evaluate(plan, src))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Not(pub Box<dyn Predicate>);
#[typetag::serde]
impl Predicate for Not {
    fn evaluate(&self, plan: &Plan, src: &HashSet<String>) -> bool {
        !self.0.evaluate(plan, src)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Xor(pub Vec<Box<dyn Predicate>>);
#[typetag::serde]
impl Predicate for Xor {
    fn evaluate(&self, plan: &Plan, src: &HashSet<String>) -> bool {
        0 != 1 & self
            .0
            .iter()
            .filter(|pred| pred.evaluate(plan, src))
            .count()
    }
}

#[derive(Serialize, Deserialize)]
pub struct Nand(pub Vec<Box<dyn Predicate>>);
#[typetag::serde]
impl Predicate for Nand {
    fn evaluate(&self, plan: &Plan, src: &HashSet<String>) -> bool {
        !self.0.iter().all(|pred| pred.evaluate(plan, src))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Nor(pub Vec<Box<dyn Predicate>>);
#[typetag::serde]
impl Predicate for Nor {
    fn evaluate(&self, plan: &Plan, src: &HashSet<String>) -> bool {
        !self.0.iter().any(|pred| pred.evaluate(plan, src))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Xnor(pub Vec<Box<dyn Predicate>>);
#[typetag::serde]
impl Predicate for Xnor {
    fn evaluate(&self, plan: &Plan, src: &HashSet<String>) -> bool {
        0 == 1 & self
            .0
            .iter()
            .filter(|pred| pred.evaluate(plan, src))
            .count()
    }
}

#[derive(Serialize, Deserialize)]
pub struct AllSuccess;
#[typetag::serde]
impl Predicate for AllSuccess {
    fn evaluate(&self, plan: &Plan, src: &HashSet<String>) -> bool {
        if src.is_empty() {
            plan.plans.iter().all(|p| p.status.unwrap_or(false))
        } else {
            src.iter()
                .filter_map(|p| plan.get(p))
                .all(|p| p.status.unwrap_or(false))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct AnySuccess;
#[typetag::serde]
impl Predicate for AnySuccess {
    fn evaluate(&self, plan: &Plan, src: &HashSet<String>) -> bool {
        if src.is_empty() {
            plan.plans.iter().any(|p| p.status.unwrap_or(false))
        } else {
            src.iter()
                .filter_map(|p| plan.get(p))
                .any(|p| p.status.unwrap_or(false))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct AllFailure;
#[typetag::serde]
impl Predicate for AllFailure {
    fn evaluate(&self, plan: &Plan, src: &HashSet<String>) -> bool {
        !if src.is_empty() {
            plan.plans.iter().any(|p| p.status.unwrap_or(true))
        } else {
            src.iter()
                .filter_map(|p| plan.get(p))
                .any(|p| p.status.unwrap_or(true))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct AnyFailure;
#[typetag::serde]
impl Predicate for AnyFailure {
    fn evaluate(&self, plan: &Plan, src: &HashSet<String>) -> bool {
        !if src.is_empty() {
            plan.plans.iter().all(|p| p.status.unwrap_or(true))
        } else {
            src.iter()
                .filter_map(|p| plan.get(p))
                .all(|p| p.status.unwrap_or(true))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::predicate::*;

    #[test]
    fn and() {
        let p = Plan::new(Box::new(DefaultBehaviour), "", false, Duration::new(0, 0));
        assert!(!And(vec![Box::new(False), Box::new(False)]).evaluate(&p, &HashSet::new()));
        assert!(!And(vec![Box::new(False), Box::new(True)]).evaluate(&p, &HashSet::new()));
        assert!(!And(vec![Box::new(True), Box::new(False)]).evaluate(&p, &HashSet::new()));
        assert!(And(vec![Box::new(True), Box::new(True)]).evaluate(&p, &HashSet::new()));
    }

    #[test]
    fn or() {
        let p = Plan::new(Box::new(DefaultBehaviour), "", false, Duration::new(0, 0));
        assert!(!Or(vec![Box::new(False), Box::new(False)]).evaluate(&p, &HashSet::new()));
        assert!(Or(vec![Box::new(False), Box::new(True)]).evaluate(&p, &HashSet::new()));
        assert!(Or(vec![Box::new(True), Box::new(False)]).evaluate(&p, &HashSet::new()));
        assert!(Or(vec![Box::new(True), Box::new(True)]).evaluate(&p, &HashSet::new()));
    }

    #[test]
    fn not() {
        let p = Plan::new(Box::new(DefaultBehaviour), "", false, Duration::new(0, 0));
        assert!(!Not(Box::new(True)).evaluate(&p, &HashSet::new()));
        assert!(Not(Box::new(False)).evaluate(&p, &HashSet::new()));
    }

    #[test]
    fn xor() {
        let p = Plan::new(Box::new(DefaultBehaviour), "", false, Duration::new(0, 0));
        assert!(!Xor(vec![Box::new(False), Box::new(False)]).evaluate(&p, &HashSet::new()));
        assert!(Xor(vec![Box::new(False), Box::new(True)]).evaluate(&p, &HashSet::new()));
        assert!(Xor(vec![Box::new(True), Box::new(False)]).evaluate(&p, &HashSet::new()));
        assert!(!Xor(vec![Box::new(True), Box::new(True)]).evaluate(&p, &HashSet::new()));
    }

    #[test]
    fn nand() {
        let p = Plan::new(Box::new(DefaultBehaviour), "", false, Duration::new(0, 0));
        assert!(Nand(vec![Box::new(False), Box::new(False)]).evaluate(&p, &HashSet::new()));
        assert!(Nand(vec![Box::new(False), Box::new(True)]).evaluate(&p, &HashSet::new()));
        assert!(Nand(vec![Box::new(True), Box::new(False)]).evaluate(&p, &HashSet::new()));
        assert!(!Nand(vec![Box::new(True), Box::new(True)]).evaluate(&p, &HashSet::new()));
    }

    #[test]
    fn nor() {
        let p = Plan::new(Box::new(DefaultBehaviour), "", false, Duration::new(0, 0));
        assert!(Nor(vec![Box::new(False), Box::new(False)]).evaluate(&p, &HashSet::new()));
        assert!(!Nor(vec![Box::new(False), Box::new(True)]).evaluate(&p, &HashSet::new()));
        assert!(!Nor(vec![Box::new(True), Box::new(False)]).evaluate(&p, &HashSet::new()));
        assert!(!Nor(vec![Box::new(True), Box::new(True)]).evaluate(&p, &HashSet::new()));
    }

    #[test]
    fn xnor() {
        let p = Plan::new(Box::new(DefaultBehaviour), "", false, Duration::new(0, 0));
        assert!(Xnor(vec![Box::new(False), Box::new(False)]).evaluate(&p, &HashSet::new()));
        assert!(!Xnor(vec![Box::new(False), Box::new(True)]).evaluate(&p, &HashSet::new()));
        assert!(!Xnor(vec![Box::new(True), Box::new(False)]).evaluate(&p, &HashSet::new()));
        assert!(Xnor(vec![Box::new(True), Box::new(True)]).evaluate(&p, &HashSet::new()));
    }

    fn make_plan(a: bool, b: bool, c: Option<bool>) -> Plan {
        let mut p = Plan::new(Box::new(DefaultBehaviour), "", false, Duration::new(0, 0));
        p.insert(Plan::new(
            Box::new(DefaultBehaviour),
            "a",
            false,
            Duration::new(0, 0),
        ));
        p.insert(Plan::new(
            Box::new(DefaultBehaviour),
            "b",
            false,
            Duration::new(0, 0),
        ));
        p.insert(Plan::new(
            Box::new(DefaultBehaviour),
            "c",
            false,
            Duration::new(0, 0),
        ));
        p.get_mut("a").unwrap().status = Some(a);
        p.get_mut("b").unwrap().status = Some(b);
        p.get_mut("c").unwrap().status = c;
        p
    }

    #[test]
    fn all_success() {
        let op = AllSuccess;
        let src = HashSet::<String>::from(["a".into(), "b".into(), "c".into()]);
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
        let src = HashSet::<String>::from(["a".into(), "b".into(), "c".into()]);
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
        let src = HashSet::<String>::from(["a".into(), "b".into(), "c".into()]);
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
        let src = HashSet::<String>::from(["a".into(), "b".into(), "c".into()]);
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
