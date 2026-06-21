let values = [1, 2];
values.push(3);
let popped = values.pop();
values.push(4);

let mapped = values.map(function (value) {
  return value * 2;
});
let filtered = mapped.filter(function (value) {
  return value > 4;
});
let obj = { name: "gts", count: filtered.length };

println(`arrays-objects=${values.length}:${popped}:${filtered.join("|")}:${obj.name}:${obj.count}`);
