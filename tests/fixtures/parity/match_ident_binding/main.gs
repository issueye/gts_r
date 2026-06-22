let value = "gts";
let label = match (value) {
  captured => `id:${captured}`,
};

println(`match-ident-binding=${label}`);
