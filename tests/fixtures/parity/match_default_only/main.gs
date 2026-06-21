let value = 99;
let label = match (value) {
  _ => "fallback",
};

println(`match-default-only=${label}`);
