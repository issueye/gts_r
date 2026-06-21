class Base {
  greet() {
    return "base";
  }
  value() {
    return 6;
  }
}

// A subclass that overrides inherited methods and calls the parent
// implementation via super.method(). This exercises the eval_call dispatch
// path for `super.method()` (not just super.method as a value).
class Child extends Base {
  greet() {
    return "child:" + super.greet();
  }
  value() {
    return super.value() + 100;
  }
}

let child = new Child();
println(`class-super-method-override=${child.greet()}:${child.value()}`);
