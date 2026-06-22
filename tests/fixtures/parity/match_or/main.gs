let value = "blue";
let label = match (value) {
  "red" | "blue" => "primary",
  "green" => "secondary",
  _ => "other",
};

println(`match-or=${label}`);
