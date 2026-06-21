class Base {
  value() {
    return 6;
  }
}

class Child extends Base {
  extra() {
    return 4;
  }
}

let child = new Child();
println(`class-inheritance-method=${child.value() + child.extra()}`);
