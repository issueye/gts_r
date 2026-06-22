function testFn() {
  return 1;
}

class TestClass {
  constructor() {
    this.value = 1;
  }
}

println(`typeof-values=${typeof 42}:${typeof "hello"}:${typeof true}:${typeof null}:${typeof undefined}:${typeof []}:${typeof {}}:${typeof testFn}:${typeof TestClass}`);
