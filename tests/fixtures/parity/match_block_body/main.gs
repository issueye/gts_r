let value = 3;
let label = match (value) {
  3 => {
    let doubled = value * 2;
    `hit:${doubled}`;
  },
  _ => "miss",
};

println(`match-block-body=${label}`);
