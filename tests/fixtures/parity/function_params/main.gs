function label(value = "item") {
  return value;
}

println(`function-params=${label()}:${label(undefined)}:${label("key")}`);
