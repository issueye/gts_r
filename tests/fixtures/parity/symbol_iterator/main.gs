let arr_it = [1, 2][Symbol.iterator]();
let a1 = arr_it.next();
let a2 = arr_it.next();
let a3 = arr_it.next();

let str_it = "go"[Symbol.iterator]();
let s1 = str_it.next();
let s2 = str_it.next();
let s3 = str_it.next();

let from_arr = Array.from("xy");

println(
  `symbol-iterator=${a1.value}:${a2.value}:${a3.done}:${s1.value}${s2.value}:${s3.done}:${from_arr.join("")}`
);
