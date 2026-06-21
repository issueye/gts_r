class Bag {
  constructor() {
    this.items = [];
  }

  add(value) {
    this.items.push(value);
    return this.items.length;
  }
}

let bag = new Bag();
bag.add("a");
println(`class-method-this=${bag.add("b")}:${bag.items.join("")}`);
