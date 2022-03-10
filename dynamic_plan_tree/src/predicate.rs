use crate::*;

#[typetag::serde]
pub trait Predicate: Send {
    fn evaluate(&self, t: &Plan) -> bool;
}

#[derive(Serialize, Deserialize)]
pub struct True;
#[typetag::serde]
impl Predicate for True {
    fn evaluate(&self, _: &Plan) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize)]
pub struct False;
#[typetag::serde]
impl Predicate for False {
    fn evaluate(&self, _: &Plan) -> bool {
        false
    }
}

#[derive(Serialize, Deserialize)]
pub struct And(pub Vec<Box<dyn Predicate>>);
#[typetag::serde]
impl Predicate for And {
    fn evaluate(&self, t: &Plan) -> bool {
        self.0.iter().all(|pred| pred.evaluate(t))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Or(pub Vec<Box<dyn Predicate>>);
#[typetag::serde]
impl Predicate for Or {
    fn evaluate(&self, t: &Plan) -> bool {
        self.0.iter().any(|pred| pred.evaluate(t))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Not(pub Box<dyn Predicate>);
#[typetag::serde]
impl Predicate for Not {
    fn evaluate(&self, t: &Plan) -> bool {
        !self.0.evaluate(t)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Xor(pub Vec<Box<dyn Predicate>>);
#[typetag::serde]
impl Predicate for Xor {
    fn evaluate(&self, t: &Plan) -> bool {
        0 != 1 & self.0.iter().filter(|pred| pred.evaluate(t)).count()
    }
}

#[derive(Serialize, Deserialize)]
pub struct Nand(pub Vec<Box<dyn Predicate>>);
#[typetag::serde]
impl Predicate for Nand {
    fn evaluate(&self, t: &Plan) -> bool {
        !self.0.iter().all(|pred| pred.evaluate(t))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Nor(pub Vec<Box<dyn Predicate>>);
#[typetag::serde]
impl Predicate for Nor {
    fn evaluate(&self, t: &Plan) -> bool {
        !self.0.iter().any(|pred| pred.evaluate(t))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Xnor(pub Vec<Box<dyn Predicate>>);
#[typetag::serde]
impl Predicate for Xnor {
    fn evaluate(&self, t: &Plan) -> bool {
        0 == 1 & self.0.iter().filter(|pred| pred.evaluate(t)).count()
    }
}

#[cfg(test)]
mod tests {
    use crate::predicate::*;

    #[test]
    fn and() {
        let p: Plan = Plan::new(
            Box::new(DefaultStateMachine),
            "",
            false,
            Duration::new(0, 0),
        );
        assert!(!And(vec![Box::new(False), Box::new(False)]).evaluate(&p));
        assert!(!And(vec![Box::new(False), Box::new(True)]).evaluate(&p));
        assert!(!And(vec![Box::new(True), Box::new(False)]).evaluate(&p));
        assert!(And(vec![Box::new(True), Box::new(True)]).evaluate(&p));
    }

    #[test]
    fn or() {
        let p: Plan = Plan::new(
            Box::new(DefaultStateMachine),
            "",
            false,
            Duration::new(0, 0),
        );
        assert!(!Or(vec![Box::new(False), Box::new(False)]).evaluate(&p));
        assert!(Or(vec![Box::new(False), Box::new(True)]).evaluate(&p));
        assert!(Or(vec![Box::new(True), Box::new(False)]).evaluate(&p));
        assert!(Or(vec![Box::new(True), Box::new(True)]).evaluate(&p));
    }

    #[test]
    fn not() {
        let p: Plan = Plan::new(
            Box::new(DefaultStateMachine),
            "",
            false,
            Duration::new(0, 0),
        );
        assert!(!Not(Box::new(True)).evaluate(&p));
        assert!(Not(Box::new(False)).evaluate(&p));
    }

    #[test]
    fn xor() {
        let p: Plan = Plan::new(
            Box::new(DefaultStateMachine),
            "",
            false,
            Duration::new(0, 0),
        );
        assert!(!Xor(vec![Box::new(False), Box::new(False)]).evaluate(&p));
        assert!(Xor(vec![Box::new(False), Box::new(True)]).evaluate(&p));
        assert!(Xor(vec![Box::new(True), Box::new(False)]).evaluate(&p));
        assert!(!Xor(vec![Box::new(True), Box::new(True)]).evaluate(&p));
    }

    #[test]
    fn nand() {
        let p: Plan = Plan::new(
            Box::new(DefaultStateMachine),
            "",
            false,
            Duration::new(0, 0),
        );
        assert!(Nand(vec![Box::new(False), Box::new(False)]).evaluate(&p));
        assert!(Nand(vec![Box::new(False), Box::new(True)]).evaluate(&p));
        assert!(Nand(vec![Box::new(True), Box::new(False)]).evaluate(&p));
        assert!(!Nand(vec![Box::new(True), Box::new(True)]).evaluate(&p));
    }

    #[test]
    fn nor() {
        let p: Plan = Plan::new(
            Box::new(DefaultStateMachine),
            "",
            false,
            Duration::new(0, 0),
        );
        assert!(Nor(vec![Box::new(False), Box::new(False)]).evaluate(&p));
        assert!(!Nor(vec![Box::new(False), Box::new(True)]).evaluate(&p));
        assert!(!Nor(vec![Box::new(True), Box::new(False)]).evaluate(&p));
        assert!(!Nor(vec![Box::new(True), Box::new(True)]).evaluate(&p));
    }

    #[test]
    fn xnor() {
        let p: Plan = Plan::new(
            Box::new(DefaultStateMachine),
            "",
            false,
            Duration::new(0, 0),
        );
        assert!(Xnor(vec![Box::new(False), Box::new(False)]).evaluate(&p));
        assert!(!Xnor(vec![Box::new(False), Box::new(True)]).evaluate(&p));
        assert!(!Xnor(vec![Box::new(True), Box::new(False)]).evaluate(&p));
        assert!(Xnor(vec![Box::new(True), Box::new(True)]).evaluate(&p));
    }
}
