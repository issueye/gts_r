function explode() {
  throw "boom";
}

let label = "none";
try {
  explode();
  label = "miss";
} catch (err) {
  label = err.message;
} finally {
  label = label + ":finally";
}

println(`try-catch=${label}`);
