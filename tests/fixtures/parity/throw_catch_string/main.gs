let label = "start";
try {
  throw "boom";
} catch (err) {
  label = err.message;
}

println(`throw-catch-string=${label}`);
