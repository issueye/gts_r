let total = 0;
for (let i = 0; i < 8; i = i + 1) {
  if (i === 4) {
    break;
  }
  total = total + i;
}

println(`for-break=${total}`);
