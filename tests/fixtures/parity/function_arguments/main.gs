function describe(first) {
  return `${arguments.length}:${arguments[0]}:${arguments[2]}:${first}`;
}

println(`function-arguments=${describe("a", "b", "c")}`);
