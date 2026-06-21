let label = "body";
try {
  label = label + ":try";
} finally {
  label = label + ":finally";
}

println(`try-finally-no-throw=${label}`);
