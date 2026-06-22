function makeCounter() {
  let count = 0;
  return function () {
    count = count + 1;
    return count;
  };
}

let next = makeCounter();
println(`closure-counter=${next()}:${next()}:${next()}`);
