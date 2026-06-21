let values = [1, 2, 3, 4];
let total = values.reduce(function (sum, value) {
  return sum + value;
}, 0);

println(`array-reduce=${total}`);
