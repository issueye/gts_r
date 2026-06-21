class Base {
  constructor(start) {
    this.value = start;
  }
}

class Child extends Base {
  constructor(start, extra) {
    super(start);
    this.value = this.value + extra;
  }
}

let child = new Child(5, 7);
println(`class-inheritance-constructor=${child.value}`);
