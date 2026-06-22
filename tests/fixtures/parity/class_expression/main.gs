let Box = class Box {
  constructor(value) {
    this.value = value;
  }

  get() {
    return this.value;
  }
};

let box = new Box(9);
println(`class-expression=${box.get()}`);
