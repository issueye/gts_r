class Counter {
  constructor(start) {
    this.value = start;
  }

  inc(step) {
    this.value = this.value + step;
    return this.value;
  }
}

let counter = new Counter(4);
println(`class-basic=${counter.inc(3)}:${counter.value}`);
