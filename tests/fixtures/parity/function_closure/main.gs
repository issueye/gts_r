function makeAdder(base) {
  return function (value) {
    return base + value;
  };
}

let addFive = makeAdder(5);
println(`function-closure=${addFive(8)}`);
