import { nextTick } from "vue";
import App from "@oxide-vue/App.vue";
import router from "@oxide-vue/router";
import pinia from "@oxide-vue/store";
import { useArticleStore } from "@oxide-vue/store/article";
import { renderer, node, snapshot } from "./vue_host.js";

const articles = Array.from({ length: 60 }, (_, i) => ({
  slug: `article-${i}`, title: `Article ${i}`, description: `Summary ${i}`,
  body: `Body ${i}`, tagList: [i % 2 ? "odd" : "even"],
  createdAt: "2020-01-01T00:00:00.000Z", updatedAt: "2020-01-01T00:00:00.000Z",
  favorited: false, favoritesCount: i % 7,
  author: { username: `author-${i % 5}`, bio: "", image: "", following: false }
}));
const comments = [];
const storage = {};
globalThis.window = {
  localStorage: {
    getItem: key => storage[key] ?? null,
    setItem: (key, value) => { storage[key] = String(value); },
    removeItem: key => { delete storage[key]; }
  }
};
globalThis.queueMicrotask = callback => Promise.resolve().then(callback);
globalThis.console ||= {};
globalThis.console.warn ||= () => {};
globalThis.console.error ||= () => {};
if (!globalThis.URLSearchParams) globalThis.URLSearchParams = class {
  constructor(input = "") {
    this.values = {};
    if (typeof input === "string") {
      for (const pair of input.replace(/^\?/, "").split("&")) {
        if (!pair) continue;
        const [key, value = ""] = pair.split("=");
        this.values[decodeURIComponent(key)] = decodeURIComponent(value);
      }
    } else Object.assign(this.values, input);
  }
  get(key) { return this.values[key] ?? null; }
  toString() { return Object.keys(this.values).map(key => `${encodeURIComponent(key)}=${encodeURIComponent(this.values[key])}`).join("&"); }
};
globalThis.fetch = async (url, options = {}) => {
  const path = String(url).replace(/^https?:\/\/[^/]+/, "").replace(/^\/api\//, "");
  const params = new URLSearchParams(path.split("?")[1] || "");
  let result;
  if (path.startsWith("articles?")) {
    const tag = params.get("tag");
    const author = params.get("author");
    const filtered = articles.filter(a => (!tag || a.tagList.includes(tag)) &&
      (!author || a.author.username === author));
    const offset = Number(params.get("offset") || 0);
    const limit = Number(params.get("limit") || 10);
    result = { articles: filtered.slice(offset, offset + limit), articlesCount: filtered.length };
  } else if (path === "tags") {
    result = { tags: ["even", "odd"] };
  } else if (/^articles\/[^/]+\/favorite$/.test(path)) {
    const article = articles.find(a => a.slug === path.split("/")[1]);
    article.favorited = options.method !== "DELETE";
    result = { article };
  } else if (/^articles\/[^/]+\/comments$/.test(path)) {
    if (options.method === "POST") {
      const comment = { id: 1, body: JSON.parse(options.body).comment.body,
        createdAt: articles[3].createdAt, author: { username: "viewer", image: "" } };
      comments.push(comment);
      result = { comment };
    } else result = { comments };
  } else if (path.startsWith("articles/")) {
    result = { article: articles.find(a => a.slug === path.split("/")[1]) };
  } else if (path.startsWith("profiles/")) {
    result = { profile: { username: path.split("/")[1], bio: "", image: "", following: false } };
  } else {
    throw new Error(`unseeded RealWorld API request: ${path}`);
  }
  return { ok: true, status: 200, json: async () => result };
};

async function settle() {
  for (let i = 0; i < 16; i++) { await Promise.resolve(); await nextTick(); }
}
async function run() {
  const root = node("root");
  const app = renderer.createApp(App);
  app.config.errorHandler = error => { throw error; };
  app.use(pinia);
  app.use(router);
  await router.push("/");
  app.mount(root);
  const records = [];
  async function phase(name, route) {
    if (route) await router.push(route);
    await settle();
    records.push({ name, ...snapshot(root), favorited: articles[3].favorited,
      comments: comments.length });
  }
  await phase("home");
  await phase("tag", "/tag/even");
  await phase("page", "/tag/even?page=2");
  await phase("article", "/article/article-3");
  await useArticleStore(pinia).addFavorite("article-3");
  await phase("favorite");
  await useArticleStore(pinia).createComment({ slug: "article-3", comment: "seeded comment" });
  await phase("comment");
  await phase("profile", "/profile/author-3");
  app.unmount();
  print(JSON.stringify(records));
}
run().catch(error => { print(String(error.stack || error)); throw error; });
