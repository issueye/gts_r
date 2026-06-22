let source = {};
source[Symbol.iterator] = function () {
  let i = 0;
  return {
    next: function () {
      i = i + 1;
      if (i <= 3) {
        return { value: i * 10, done: false };
      }
      return { value: undefined, done: true };
    },
  };
};

let sum = 0;
for (let value of source) {
  sum = sum + value;
}

let copied = Array.from(source);

println(`custom-symbol-iterator=${sum}:${copied.join("|")}`);
