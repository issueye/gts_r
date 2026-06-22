let sum = 0;
for (let value of [1, 2, 3]) {
  sum = sum + value;
}

let word = "";
for (let ch of "go") {
  word = word + ch;
}

let map = new Map([["a", 2], ["b", 3]]);
let map_total = 0;
for (let value of map) {
  map_total = map_total + value;
}

let set = new Set(["x", "y"]);
let set_text = "";
for (let value of set) {
  set_text = set_text + value;
}

println(`for-of-array=${sum}:${word}:${map_total}:${set_text}`);
