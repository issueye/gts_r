let counter = {
  value: 4,
  inc: function (step) {
    this.value = this.value + step;
    return this.value;
  },
};

println(`object-method-call=${counter.inc(6)}:${counter.value}`);
