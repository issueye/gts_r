let inner = require("./inner/value");

export function combined(extra) {
  return inner.value() + extra;
}
