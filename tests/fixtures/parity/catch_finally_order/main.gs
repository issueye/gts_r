let label = "start";
try {
  throw "hit";
} catch (err) {
  label = `${label}:catch`;
} finally {
  label = `${label}:finally`;
}

println(`catch-finally-order=${label}`);
