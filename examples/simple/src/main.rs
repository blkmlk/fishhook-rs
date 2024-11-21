fn main() {
    let v = Box::new(10u8);
    let a = 10;
    let b = foo(&a);
    {
        let c = Box::new(b);
    }
    let v = String::from("sukab");
}

fn foo(x: &i32) -> i32 {
    *x
}
