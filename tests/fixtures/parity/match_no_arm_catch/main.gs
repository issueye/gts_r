let label = "none";
try {
  match (7) {
    1 => "one",
  };
} catch (err) {
  label = err.name;
}

println(`match-no-arm-catch=${label}`);
