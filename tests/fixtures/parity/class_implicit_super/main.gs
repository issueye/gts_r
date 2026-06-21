class Base {
  constructor() {
    this.value = 9;
  }
}

class Child extends Base {
  getValue() {
    return this.value + 1;
  }
}

let child = new Child();
println(`class-implicit-super=${child.getValue()}`);
