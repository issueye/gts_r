let i = 0;
let total = 0;
while (i < 6) {
  i = i + 1;
  if (i === 3) {
    continue;
  }
  total = total + i;
}

println(`while-continue=${total}`);
