// The benchmark API emits plain ASCII article bodies. Sanitizing their marked
// rendering is an external DOM operation, so the headless host preserves the
// value while asserting the seed cannot carry HTML markup.
export default {
  sanitize(html) {
    if (/<(?:script|style|iframe|object|embed)\b/i.test(html))
      throw new Error("unsafe benchmark article body");
    return html;
  }
};
