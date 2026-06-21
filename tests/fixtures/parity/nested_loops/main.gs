let text = "";
for (let row = 1; row <= 2; row = row + 1) {
  for (let col = 1; col <= 3; col = col + 1) {
    text = `${text}${row}${col}`;
  }
}

println(`nested-loops=${text}`);
