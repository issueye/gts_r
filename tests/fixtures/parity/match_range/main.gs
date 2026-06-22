let value = 7;
let label = match (value) {
  1..5 => "small",
  5..=10 => "medium",
  _ => "large",
};

println(`match-range=${label}`);
