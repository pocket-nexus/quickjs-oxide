import React from "react";
import { renderToStaticMarkup } from "react-dom/server.browser";
import { HomePage } from "@oxide-react/pages/home/home.ui";
import { ArticlePage } from "@oxide-react/pages/article/article.ui";
import { ProfilePage } from "@oxide-react/pages/profile/profile.ui";
import { getHomeNavigation } from "@oxide-react/pages/home/home.state";
import { getProfileNavigation } from "@oxide-react/pages/profile/profile.state";
import { setRouteState } from "./react_router_host.js";

globalThis.queueMicrotask = callback => Promise.resolve().then(callback);
globalThis.console ||= {};
globalThis.console.warn ||= () => {};
globalThis.console.error ||= () => {};
if (!globalThis.URLSearchParams) globalThis.URLSearchParams = class {
  constructor(query = "") {
    this.values = {};
    for (const pair of String(query).replace(/^\?/, "").split("&")) {
      if (!pair) continue;
      const [key, value = ""] = pair.split("=");
      this.values[decodeURIComponent(key)] = decodeURIComponent(value);
    }
  }
  get(key) { return this.values[key] ?? null; }
  set(key, value) { this.values[key] = String(value); }
  toString() { return Object.keys(this.values).map(key => `${encodeURIComponent(key)}=${encodeURIComponent(this.values[key])}`).join("&"); }
};
if (!globalThis.FormData) globalThis.FormData = class {
  constructor() { this.values = {}; }
  set(key, value) { this.values[key] = value; }
  get(key) { return this.values[key] ?? null; }
};

const articles = Array.from({ length: 60 }, (_, i) => ({
  slug: `article-${i}`, title: `Article ${i}`, description: `Summary ${i}`,
  body: `Body ${i}`, tagList: [i % 2 ? "odd" : "even"],
  createdAt: "2020-01-01T00:00:00.000Z", updatedAt: "2020-01-01T00:00:00.000Z",
  favorited: false, favoritesCount: i % 7,
  author: { username: `author-${i % 5}`, bio: "", image: "", following: false }
}));
const userData = { user: { username: "viewer", image: "" } };
const comments = [];

function snapshot(html) {
  let hash = 2166136261;
  for (let i = 0; i < html.length; i++) {
    hash ^= html.charCodeAt(i);
    hash = Math.imul(hash, 16777619) >>> 0;
  }
  return { bytes: html.length, hash: hash.toString(16) };
}

function homeLoader(tag, offset) {
  const filtered = articles.filter(article => !tag || article.tagList.includes(tag));
  const searchParams = { limit: 10, offset, ...(tag ? { tag } : {}) };
  return {
    articlesPromise: { articles: filtered.slice(offset, offset + 10), articlesCount: filtered.length },
    tagsPromise: { tags: ["even", "odd"] }, userData: null, searchParams,
    navigation: getHomeNavigation(searchParams)
  };
}
function articleLoader() {
  return { articlePromise: { article: articles[3] }, commentsPromise: { comments }, userData };
}
function profileLoader() {
  const searchParams = { limit: 5, offset: 0, author: "author-3" };
  const filtered = articles.filter(article => article.author.username === "author-3");
  return {
    profilePromise: { profile: { username: "author-3", image: "", bio: "", following: false } },
    articlesPromise: { articles: filtered.slice(0, 5), articlesCount: filtered.length },
    userData, searchParams, navigation: getProfileNavigation(searchParams)
  };
}

async function run() {
  const records = [];
  function phase(name, Component, loader, params = {}, search = "") {
    setRouteState({ loader, params, location: { search } });
    const html = renderToStaticMarkup(React.createElement(Component));
    if (!html) throw new Error(`${name}: empty React render`);
    records.push({ name, ...snapshot(html) });
  }
  phase("home", HomePage, homeLoader(null, 0));
  phase("tag", HomePage, homeLoader("even", 0), {}, "?tag=even");
  phase("page", HomePage, homeLoader("even", 10), {}, "?tag=even&offset=10");
  phase("article", ArticlePage, articleLoader(), { slug: "article-3" });
  articles[3].favorited = true;
  articles[3].favoritesCount++;
  phase("favorite", ArticlePage, articleLoader(), { slug: "article-3" });
  comments.push({ id: 1, body: "seeded comment", createdAt: articles[3].createdAt,
    author: { username: "viewer", image: "" } });
  phase("comment", ArticlePage, articleLoader(), { slug: "article-3" });
  phase("profile", ProfilePage, profileLoader(), { username: "author-3" });
  print(JSON.stringify(records));
}
run().catch(error => { print(String(error.stack || error)); throw error; });
