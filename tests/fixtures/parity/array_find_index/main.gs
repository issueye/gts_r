let values = [3, 5, 8, 13];
let found = values.find(function (value) {
  return value > 6;
});
let index = values.findIndex(function (value) {
  return value === 13;
});

println(`array-find-index=${found}:${index}`);
