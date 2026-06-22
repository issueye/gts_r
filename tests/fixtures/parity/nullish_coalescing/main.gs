let missing = null ?? 42;
let absent = undefined ?? 7;
let zero = 0 ?? 9;
let nope = false ?? true;
println(`nullish-coalescing=${missing}:${absent}:${zero}:${nope}`);
