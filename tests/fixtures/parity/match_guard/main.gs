let value = 6;
let label = match (value) {
  n if n < 5 => "small",
  n if n < 10 => `medium:${n}`,
  _ => "large",
};

println(`match-guard=${label}`);
