function fact(n) {
  if (n <= 1) {
    return 1;
  }
  return n * fact(n - 1);
}

println(`recursive-function=${fact(5)}`);
