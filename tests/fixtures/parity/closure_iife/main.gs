let value = (function () {
  let prefix = "go";
  return function (suffix) {
    return prefix + suffix;
  };
})()("script");

println(`closure-iife=${value}`);
