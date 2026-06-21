let values = [];
for (let i = 0; i < 4; i = i + 1) {
  values.push(i * i);
}

println(`loop-array-build=${values.join("|")}`);
