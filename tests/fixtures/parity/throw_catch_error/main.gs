let label = "none";
try {
  throw new Error("boom");
} catch (err) {
  label = err.message;
}

println(`throw-catch-error=${label}`);
