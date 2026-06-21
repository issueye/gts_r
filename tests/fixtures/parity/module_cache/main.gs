let a = require("./lib/counter");
let b = require("./lib/counter");

println(`module-cache=${a.value()}:${b.value()}`);
