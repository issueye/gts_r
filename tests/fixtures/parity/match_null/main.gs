let value = null;
let label = match (value) {
  null => "nil",
  _ => "other",
};

println(`match-null=${label}`);
