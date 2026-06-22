function collect(prefix, ...items) {
  return `${prefix}:${items.join("|")}:${items.length}`;
}

println(`function-rest-params=${collect("v", 1, 2, 3)}`);
