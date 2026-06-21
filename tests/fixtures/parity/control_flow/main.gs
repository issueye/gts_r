let total = 0;
if (total === 0) {
  total = total + 1;
} else {
  total = total + 100;
}

let i = 0;
while (i < 3) {
  total = total + i;
  i = i + 1;
}

for (let j = 0; j < 4; j = j + 1) {
  if (j === 2) {
    continue;
  }
  total = total + j;
}

println(`control-flow=${total}`);
