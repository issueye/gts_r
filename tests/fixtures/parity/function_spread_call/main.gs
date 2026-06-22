function join3(a, b, c) {
  return `${a}:${b}:${c}`;
}

let tail = ["b", "c"];
println(`function-spread-call=${join3("a", ...tail)}`);
