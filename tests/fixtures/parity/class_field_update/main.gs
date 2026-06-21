class Box {
  constructor(value) {
    this.value = value;
  }
}

let box = new Box(3);
box.value = box.value * 4;

println(`class-field-update=${box.value}`);
