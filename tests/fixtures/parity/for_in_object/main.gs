let seen = "";
let obj = { a: 1, b: 2, c: 3 };

for (let key in obj) {
  seen = seen + key;
}

println(`for-in-object=${seen}`);
