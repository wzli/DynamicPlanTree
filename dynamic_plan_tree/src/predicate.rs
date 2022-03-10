use serde::{Deserialize, Serialize};

#[typetag::serialize(tag = "type")]
trait TestTrait<T> {}

#[derive(Serialize, Deserialize)]
struct TestStruct(i32);
#[typetag::serialize]
impl TestTrait<bool> for TestStruct {}

#[derive(Serialize, Deserialize)]
struct TestStruct2(f32);
#[typetag::serialize]
impl TestTrait<bool> for TestStruct2 {}

trait Predicate<T> {
    fn evaluate(&self, t: &T) -> bool;
}

struct True;
impl<T> Predicate<T> for True {
    fn evaluate(&self, _: &T) -> bool {
        true
    }
}

struct False;
impl<T> Predicate<T> for False {
    fn evaluate(&self, _: &T) -> bool {
        false
    }
}

struct And<T>(Vec<Box<dyn Predicate<T>>>);
impl<T> Predicate<T> for And<T> {
    fn evaluate(&self, t: &T) -> bool {
        self.0.iter().all(|pred| pred.evaluate(t))
    }
}

struct Or<T>(Vec<Box<dyn Predicate<T>>>);
impl<T> Predicate<T> for Or<T> {
    fn evaluate(&self, t: &T) -> bool {
        self.0.iter().any(|pred| pred.evaluate(t))
    }
}

struct Not<T>(Box<dyn Predicate<T>>);
impl<T> Predicate<T> for Not<T> {
    fn evaluate(&self, t: &T) -> bool {
        !self.0.evaluate(t)
    }
}

struct Xor<T>(Vec<Box<dyn Predicate<T>>>);
impl<T> Predicate<T> for Xor<T> {
    fn evaluate(&self, t: &T) -> bool {
        0 != 1 & self.0.iter().filter(|pred| pred.evaluate(t)).count()
    }
}

struct Nand<T>(Vec<Box<dyn Predicate<T>>>);
impl<T> Predicate<T> for Nand<T> {
    fn evaluate(&self, t: &T) -> bool {
        !self.0.iter().all(|pred| pred.evaluate(t))
    }
}

struct Nor<T>(Vec<Box<dyn Predicate<T>>>);
impl<T> Predicate<T> for Nor<T> {
    fn evaluate(&self, t: &T) -> bool {
        !self.0.iter().any(|pred| pred.evaluate(t))
    }
}

struct Xnor<T>(Vec<Box<dyn Predicate<T>>>);
impl<T> Predicate<T> for Xnor<T> {
    fn evaluate(&self, t: &T) -> bool {
        0 == 1 & self.0.iter().filter(|pred| pred.evaluate(t)).count()
    }
}

struct Transition {
    src: Vec<String>,
    dst: Vec<String>,
    pred: Box<dyn Predicate<bool>>,
}

impl Transition {
    fn new(pred: Box<dyn Predicate<bool>>) -> Self {
        Self {
            src: Vec::new(),
            dst: Vec::new(),
            pred,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::predicate::*;

    struct AB {
        pub a: bool,
        pub b: bool,
    }

    struct IsA;
    impl Predicate<AB> for IsA {
        fn evaluate(&self, ab: &AB) -> bool {
            ab.a
        }
    }

    struct IsB;
    impl Predicate<AB> for IsB {
        fn evaluate(&self, ab: &AB) -> bool {
            ab.b
        }
    }

    #[test]
    fn and() {
        let is_a = Box::new(IsA);
        let is_b = Box::new(IsB);
        let op = And(vec![is_a, is_b]);
        assert!(!op.evaluate(&AB { a: false, b: false }));
        assert!(!op.evaluate(&AB { a: false, b: true }));
        assert!(!op.evaluate(&AB { a: true, b: false }));
        assert!(op.evaluate(&AB { a: true, b: true }));
    }

    #[test]
    fn or() {
        let is_a = Box::new(IsA);
        let is_b = Box::new(IsB);
        let op = Or(vec![is_a, is_b]);
        assert!(!op.evaluate(&AB { a: false, b: false }));
        assert!(op.evaluate(&AB { a: false, b: true }));
        assert!(op.evaluate(&AB { a: true, b: false }));
        assert!(op.evaluate(&AB { a: true, b: true }));
    }

    #[test]
    fn not() {
        assert!(!Not::<()>(Box::new(True)).evaluate(&()));
        assert!(Not::<()>(Box::new(False)).evaluate(&()));
    }

    #[test]
    fn xor() {
        let is_a = Box::new(IsA);
        let is_b = Box::new(IsB);
        let op = Xor(vec![is_a, is_b]);
        assert!(!op.evaluate(&AB { a: false, b: false }));
        assert!(op.evaluate(&AB { a: false, b: true }));
        assert!(op.evaluate(&AB { a: true, b: false }));
        assert!(!op.evaluate(&AB { a: true, b: true }));
    }

    #[test]
    fn nand() {
        let is_a = Box::new(IsA);
        let is_b = Box::new(IsB);
        let op = Nand(vec![is_a, is_b]);
        assert!(op.evaluate(&AB { a: false, b: false }));
        assert!(op.evaluate(&AB { a: false, b: true }));
        assert!(op.evaluate(&AB { a: true, b: false }));
        assert!(!op.evaluate(&AB { a: true, b: true }));
    }

    #[test]
    fn nor() {
        let is_a = Box::new(IsA);
        let is_b = Box::new(IsB);
        let op = Nor(vec![is_a, is_b]);
        assert!(op.evaluate(&AB { a: false, b: false }));
        assert!(!op.evaluate(&AB { a: false, b: true }));
        assert!(!op.evaluate(&AB { a: true, b: false }));
        assert!(!op.evaluate(&AB { a: true, b: true }));
    }

    #[test]
    fn xnor() {
        let is_a = Box::new(IsA);
        let is_b = Box::new(IsB);
        let op = Xnor(vec![is_a, is_b]);
        assert!(op.evaluate(&AB { a: false, b: false }));
        assert!(!op.evaluate(&AB { a: false, b: true }));
        assert!(!op.evaluate(&AB { a: true, b: false }));
        assert!(op.evaluate(&AB { a: true, b: true }));
    }

    #[test]
    fn serde() {
        let x: Box<dyn TestTrait<bool>> = Box::new(TestStruct(11));
        let y = TestStruct2(0.9);
        let json = serde_yaml::to_string(&x).unwrap();
        println!("{json}");
        let json = serde_json::to_string(&y).unwrap();
        println!("{json}");
    }
}
