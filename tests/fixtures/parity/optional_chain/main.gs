let obj = {
  name: "gts",
  nested: {
    value: 7,
  },
  add: (value) => value + 1,
};

let missing = null;
let key = "value";
let name = obj?.name;
let value = obj.nested?.[key];
let absent = missing?.name;
let called = obj.add?.(4);

println(`optional-chain=${name}:${value}:${absent}:${called}`);
