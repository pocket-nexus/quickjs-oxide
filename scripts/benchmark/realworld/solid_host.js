import { createRenderer } from "solid-js/universal";

export function node(kind, value = "") {
  return { kind, value, props: {}, parent: null, children: [] };
}

function detach(child) {
  if (child.parent) {
    const siblings = child.parent.children;
    const index = siblings.indexOf(child);
    if (index >= 0) siblings.splice(index, 1);
    child.parent = null;
  }
}

export const {
  render, effect, memo, createComponent, createElement, createTextNode,
  insertNode, insert, spread, setProp, mergeProps
} = createRenderer({
  createElement: kind => node(kind),
  createTextNode: value => node("#text", String(value)),
  replaceText: (target, value) => { target.value = String(value); },
  setProperty: (target, name, value) => { target.props[name] = value; },
  insertNode(parent, child, anchor) {
    detach(child);
    const index = anchor ? parent.children.indexOf(anchor) : -1;
    parent.children.splice(index < 0 ? parent.children.length : index, 0, child);
    child.parent = parent;
  },
  isTextNode: target => target.kind === "#text",
  removeNode: (_parent, child) => detach(child),
  getParentNode: target => target.parent,
  getFirstChild: target => target.children[0],
  getNextSibling(target) {
    if (!target.parent) return undefined;
    return target.parent.children[target.parent.children.indexOf(target) + 1];
  }
});

export { For, Show, Suspense, SuspenseList, Switch, Match, Index, ErrorBoundary } from "solid-js";

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
