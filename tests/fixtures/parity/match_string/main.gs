let value = "green";
let label = match (value) {
  "red" => "stop",
  "green" => "go",
  _ => "wait",
};

println(`match-string=${label}`);
