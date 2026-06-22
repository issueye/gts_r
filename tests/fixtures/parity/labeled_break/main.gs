let total = 0;

outer:
for (let i = 0; i < 3; i = i + 1) {
  if (i === 1) {
    break outer;
  }
  total = total + 1;
}

println(`labeled-break=${total}`);
