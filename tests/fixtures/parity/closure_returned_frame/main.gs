function outer() {
  let value = 21;
  function inner() {
    return value * 2;
  }
  return inner;
}

let fn = outer();
println(`closure-returned-frame=${fn()}`);
