let values = [2, 3];
values.unshift(1);
let first = values.shift();

println(`array-shift-unshift=${first}:${values.join("|")}`);
