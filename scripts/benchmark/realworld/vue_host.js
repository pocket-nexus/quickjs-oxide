import { createRenderer } from "vue";

export function node(kind, value = "") {
  return { kind, value, props: {}, parent: null, children: [] };
}

function detach(child) {
  if (!child.parent) return;
  const children = child.parent.children;
  const index = children.indexOf(child);
  if (index >= 0) children.splice(index, 1);
  child.parent = null;
}

export const renderer = createRenderer({
  createElement: kind => node(kind),
  createText: value => node("#text", String(value)),
  createComment: value => node("#comment", String(value)),
  setText: (target, value) => { target.value = String(value); },
  setElementText(target, value) {
    target.children.length = 0;
    if (value) {
      const child = node("#text", String(value));
      child.parent = target;
      target.children.push(child);
    }
  },
  patchProp: (target, key, _old, value) => { target.props[key] = value; },
  insert(child, parent, anchor) {
    detach(child);
    const index = anchor ? parent.children.indexOf(anchor) : -1;
    parent.children.splice(index < 0 ? parent.children.length : index, 0, child);
    child.parent = parent;
  },
  remove: detach,
  parentNode: target => target.parent,
  nextSibling(target) {
    if (!target.parent) return null;
    return target.parent.children[target.parent.children.indexOf(target) + 1] || null;
  }
});

export function snapshot(root) {
  let count = 0;
  let hash = 2166136261;
  function mix(text) {
    for (let i = 0; i < text.length; i++) {
      hash ^= text.charCodeAt(i);
      hash = Math.imul(hash, 16777619) >>> 0;
    }
  }
  function visit(current) {
    count++;
    mix(current.kind);
    mix(current.value);
    for (const key of Object.keys(current.props).sort()) {
      const value = current.props[key];
      if (typeof value !== "function" && value != null) {
        mix(key);
        mix(String(value));
      }
    }
    for (const child of current.children) visit(child);
  }
  visit(root);
  return { nodes: count, hash: hash.toString(16) };
}
