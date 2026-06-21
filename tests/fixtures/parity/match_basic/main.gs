let value = 2;
let label = match (value) {
  1 => "one",
  2 => "two",
  _ => "other",
};

println(`match-basic=${label}`);
