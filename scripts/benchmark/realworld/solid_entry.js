import App from "@oxide-app/src/App";
import { Provider, useStore } from "@oxide-app/src/store";
import { render, createComponent, node, snapshot } from "./solid_host.js";

const articles = Array.from({ length: 60 }, (_, i) => ({
  slug: `article-${i}`, title: `Article ${i}`, description: `Summary ${i}`,
  body: `Body ${i}`, tagList: [i % 2 ? "odd" : "even"],
  createdAt: "2020-01-01T00:00:00.000Z", updatedAt: "2020-01-01T00:00:00.000Z",
  favorited: false, favoritesCount: i % 7,
  author: { username: `author-${i % 5}`, bio: "", image: "", following: false }
}));
const comments = [];
const listeners = {};
const location = { hash: "#/" };
globalThis.console ||= {};
globalThis.console.warn = () => {};
globalThis.console.error ||= () => {};
globalThis.queueMicrotask = callback => Promise.resolve().then(callback);
if (!globalThis.URLSearchParams) globalThis.URLSearchParams = class {
  constructor(query) {
    this.values = {};
    for (const pair of String(query).replace(/^\?/, "").split("&")) {
      if (!pair) continue;
      const [key, value = ""] = pair.split("=");
      this.values[decodeURIComponent(key)] = decodeURIComponent(value);
    }
  }
  get(key) { return this.values[key] ?? null; }
};
globalThis.window = {
  location,
  addEventListener: (name, callback) => { listeners[name] = callback; },
  removeEventListener: name => { delete listeners[name]; },
  scrollTo: () => {}
};
globalThis.localStorage = { getItem: () => null, setItem: () => {}, removeItem: () => {} };
globalThis.fetch = async (url, options = {}) => {
  const path = String(url).replace(/^https?:\/\/[^/]+/, "");
  if (globalThis.__oxide_debug_fetch) print(path);
  const query = path.split("?")[1] || "";
  const params = new URLSearchParams(query);
  let result;
  if (path.startsWith("/api/articles?")) {
    const tag = params.get("tag");
    const author = params.get("author");
    const filtered = articles.filter(a => (!tag || a.tagList.includes(tag)) &&
      (!author || a.author.username === author));
    const offset = Number(params.get("offset") || 0);
    const limit = Number(params.get("limit") || 10);
    result = { articles: filtered.slice(offset, offset + limit), articlesCount: filtered.length };
  } else if (path === "/api/tags") {
    result = { tags: ["even", "odd"] };
  } else if (/^\/api\/articles\/[^/]+\/favorite$/.test(path)) {
    const article = articles.find(a => a.slug === path.split("/")[3]);
    article.favorited = options.method !== "delete";
    result = { article };
  } else if (/^\/api\/articles\/[^/]+\/comments$/.test(path)) {
    if (options.method === "post") {
      const comment = { id: 1, body: JSON.parse(options.body).comment.body,
        createdAt: articles[3].createdAt, author: { username: "viewer", image: "" } };
      comments.push(comment);
      result = { comment };
    } else result = { comments };
  } else if (path.startsWith("/api/articles/")) {
    result = { article: articles.find(a => a.slug === path.split("/")[3]) };
  } else if (path.startsWith("/api/profiles/")) {
    result = { profile: { username: path.split("/")[3], bio: "", image: "", following: false } };
  } else {
    throw new Error(`unseeded RealWorld API request: ${path}`);
  }
  return { json: async () => result };
};

async function settle() {
  for (let i = 0; i < 16; i++) await Promise.resolve();
}
async function run() {
  const root = node("root");
  let controls;
  function ObservedApp() {
    controls = useStore()[1];
    return createComponent(App, {});
  }
  const dispose = render(() => createComponent(Provider, {
    get children() { return createComponent(ObservedApp, {}); }
  }), root);
  const records = [];
  async function phase(name, hash) {
    if (hash !== undefined) {
      location.hash = hash;
      listeners.hashchange?.();
    }
    await settle();
    records.push({ name, ...snapshot(root), favorited: articles[3].favorited,
      comments: comments.length });
  }
  await phase("home");
  await phase("tag", "#/?tab=even");
  controls.setPage(1);
  controls.loadArticles({ tag: "even" });
  await phase("page");
  await phase("article", "#/article/article-3");
  await controls.makeFavorite("article-3");
  await phase("favorite");
  await controls.createComment({ body: "seeded comment" });
  await controls.loadComments("article-3", true);
  await phase("comment");
  await phase("profile", "#/@author-3");
  dispose();
  print(JSON.stringify(records));
}
run().catch(error => { print(String(error.stack || error)); throw error; });
