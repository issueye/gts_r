function add(a, b) {
  return a + b;
}

function twice(value) {
  return value * 2;
}

println(`function-call=${twice(add(2, 5))}`);